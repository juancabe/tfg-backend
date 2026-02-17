use crate::dao::DbPool;
use crate::dao::group::GroupDao;
use crate::dao::location::LocationDao;
use crate::dao::producer::{ProducerAuthCredentials, ProducerDao};
use crate::interface;
use crate::interface::types::group::Group;
use crate::interface::types::producer::Producer;
use crate::model::models::{self};
use crate::service::ServiceError;
use uuid::Uuid;

// TODO: Integration test
pub struct ProducerService<P: ProducerDao, L: LocationDao, G: GroupDao> {
    producer_dao: P,
    location_dao: L,
    group_dao: G,
}

impl<P, L, G> From<DbPool> for ProducerService<P, L, G>
where
    P: ProducerDao + Clone + From<DbPool>,
    L: LocationDao + Clone + From<DbPool>,
    G: GroupDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        // We create P and G generically
        let p_dao = P::from(value.clone());
        let l_dao = L::from(value.clone());
        let g_dao = G::from(value);

        Self::new(p_dao, l_dao, g_dao)
    }
}

impl<P: ProducerDao + Clone, L: LocationDao + Clone, G: GroupDao + Clone> ProducerService<P, L, G> {
    pub fn new(producer_dao: P, location_dao: L, group_dao: G) -> Self {
        Self {
            producer_dao,
            location_dao,
            group_dao,
        }
    }

    pub fn create_producer(
        &self,
        user_id: Uuid,
        new_producer: interface::types::producer::NewProducer,
    ) -> Result<Producer, ServiceError> {
        {
            let group = self.group_dao.get_group(new_producer.group_id)?;
            if group.owner_user_id != user_id {
                return Err(ServiceError::Forbidden);
            }
        }

        let loc = self
            .location_dao
            .create_location(new_producer.location.try_into()?)?;

        let model_new_producer = models::NewProducer {
            location_id: loc.id,
            group_id: new_producer.group_id,
            name: new_producer.name,
            description: new_producer.description,
            forwarder_id: new_producer.forwarder_id,
        };

        self.producer_dao
            .create_producer(model_new_producer)
            .map_err(Into::into)
            .map(|p| p.into())
    }

    /// Only returns the producer when the user has visibility permissions
    pub fn get_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<Producer, ServiceError> {
        let producer = self
            .producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;

        let group = self.group_dao.get_group(producer.group_id)?;

        if group.owner_user_id != user_id && !group.is_public {
            Err(ServiceError::Forbidden)
        } else {
            Ok(producer.into())
        }
    }

    pub fn get_owned_producers(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Group, Vec<Producer>)>, ServiceError> {
        self.group_dao
            .get_user_groups(user_id)?
            .into_iter()
            .map(|g| {
                let producers = self
                    .producer_dao
                    .get_by_group_id(g.id)
                    .map(|v| v.into_iter().map(Producer::from).collect())
                    .map_err(ServiceError::from)?;
                Ok((Group::from(g), producers))
            })
            .collect()
    }

    pub fn delete_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<(), ServiceError> {
        let producer = self
            .producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;
        let group = self.group_dao.get_group(producer.group_id)?;
        if user_id != group.owner_user_id {
            Err(ServiceError::Forbidden)?;
        }

        self.producer_dao.delete(producer_id)?;
        Ok(())
    }

    pub fn rotate_credentials(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<ProducerAuthCredentials, ServiceError> {
        todo!("Implement credential rotation in DAO")
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::dao::group::PgGroupDao;
    use crate::dao::location::PgLocationDao;
    use crate::dao::producer::PgProducerDao;
    use crate::interface::types::location::NewLocation;
    use crate::interface::types::producer::NewProducer;
    use crate::test_utils::{
        TestContext, create_test_group, create_test_location, create_test_producer,
        create_test_user, setup,
    };
    use diesel::prelude::*;

    fn setup_service() -> (
        TestContext,
        ProducerService<PgProducerDao, PgLocationDao, PgGroupDao>,
    ) {
        let ctx = setup();
        let service = ProducerService::from(ctx.pool.clone());
        (ctx, service)
    }

    #[test]
    fn test_new() {
        let ctx = setup();
        let p_dao = PgProducerDao::new(ctx.pool.clone());
        let l_dao = PgLocationDao::new(ctx.pool.clone());
        let g_dao = PgGroupDao::new(ctx.pool.clone());
        let _service = ProducerService::new(p_dao, l_dao, g_dao);
    }

    #[test]
    fn test_create_producer_success() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "owner");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, user.id, location.id, "group");

        let new_producer = NewProducer {
            location: NewLocation {
                name: Some("Prod Loc".to_string()),
                geo_lat: Some(1.0),
                geo_long: Some(2.0),
            },
            group_id: group.id,
            name: "New Prod".to_string(),
            description: "Desc".to_string(),
            forwarder_id: None,
        };

        let result = service
            .create_producer(user.id, new_producer)
            .expect("Failed to create producer");
        assert_eq!(result.name, "New Prod");
        assert_eq!(result.group_id, group.id);
    }

    #[test]
    fn test_create_producer_forbidden() {
        let (ctx, service) = setup_service();
        let owner = create_test_user(&ctx.pool, "owner");
        let other = create_test_user(&ctx.pool, "other");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "group");

        let new_producer = NewProducer {
            location: NewLocation {
                name: Some("Prod Loc".to_string()),
                geo_lat: None,
                geo_long: None,
            },
            group_id: group.id,
            name: "New Prod".to_string(),
            description: "Desc".to_string(),
            forwarder_id: None,
        };

        let result = service.create_producer(other.id, new_producer);
        assert!(matches!(result, Err(ServiceError::Forbidden)));
    }

    #[test]
    fn test_create_producer_group_not_found() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "user");

        let new_producer = NewProducer {
            location: NewLocation {
                name: None,
                geo_lat: None,
                geo_long: None,
            },
            group_id: Uuid::new_v4(),
            name: "New Prod".to_string(),
            description: "Desc".to_string(),
            forwarder_id: None,
        };

        let result = service.create_producer(user.id, new_producer);
        assert!(matches!(result, Err(ServiceError::DbError(_))));
    }

    #[test]
    fn test_get_producer_success_owner() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "owner");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, user.id, location.id, "group");
        let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

        let result = service
            .get_producer(user.id, producer.id)
            .expect("Failed to get producer");
        assert_eq!(result.id, producer.id);
    }

    #[test]
    fn test_get_producer_success_public() {
        let (ctx, service) = setup_service();
        let owner = create_test_user(&ctx.pool, "owner");
        let other = create_test_user(&ctx.pool, "other");
        let location = create_test_location(&ctx.pool, "loc");

        let group = {
            let mut conn = ctx.pool.get().unwrap();
            use crate::model::schema::groups::dsl::*;
            let group: crate::model::models::Group = diesel::insert_into(groups)
                .values(crate::model::models::NewGroup {
                    owner_user_id: owner.id,
                    location_id: location.id,
                    name: "public_group".to_string(),
                    description: "desc".to_string(),
                })
                .returning(crate::model::models::Group::as_returning())
                .get_result(&mut conn)
                .unwrap();

            diesel::update(groups.find(group.id))
                .set(is_public.eq(true))
                .execute(&mut conn)
                .unwrap();
            group
        };

        let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

        let result = service
            .get_producer(other.id, producer.id)
            .expect("Failed to get producer");
        assert_eq!(result.id, producer.id);
    }

    #[test]
    fn test_get_producer_forbidden() {
        let (ctx, service) = setup_service();
        let owner = create_test_user(&ctx.pool, "owner");
        let other = create_test_user(&ctx.pool, "other");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "private_group");
        let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

        let result = service.get_producer(other.id, producer.id);
        assert!(matches!(result, Err(ServiceError::Forbidden)));
    }

    #[test]
    fn test_get_producer_not_found() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "user");

        let result = service.get_producer(user.id, Uuid::new_v4());
        assert!(matches!(result, Err(ServiceError::NotFound)));
    }

    #[test]
    fn test_get_owned_producers() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "owner");
        let location = create_test_location(&ctx.pool, "loc");
        let group1 = create_test_group(&ctx.pool, user.id, location.id, "g1");
        let group2 = create_test_group(&ctx.pool, user.id, location.id, "g2");
        let p1 = create_test_producer(&ctx.pool, group1.id, location.id, "p1");
        let p2 = create_test_producer(&ctx.pool, group2.id, location.id, "p2");

        let result = service
            .get_owned_producers(user.id)
            .expect("Failed to get owned producers");
        assert_eq!(result.len(), 2);

        let g1_prods = result
            .iter()
            .find(|(g, _)| g.id == group1.id)
            .map(|(_, ps)| ps)
            .unwrap();
        assert_eq!(g1_prods.len(), 1);
        assert_eq!(g1_prods[0].id, p1.id);

        let g2_prods = result
            .iter()
            .find(|(g, _)| g.id == group2.id)
            .map(|(_, ps)| ps)
            .unwrap();
        assert_eq!(g2_prods.len(), 1);
        assert_eq!(g2_prods[0].id, p2.id);
    }

    #[test]
    fn test_delete_producer_success() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "owner");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, user.id, location.id, "group");
        let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

        service
            .delete_producer(user.id, producer.id)
            .expect("Failed to delete producer");

        let check = service.get_producer(user.id, producer.id);
        assert!(matches!(check, Err(ServiceError::NotFound)));
    }

    #[test]
    fn test_delete_producer_forbidden() {
        let (ctx, service) = setup_service();
        let owner = create_test_user(&ctx.pool, "owner");
        let other = create_test_user(&ctx.pool, "other");
        let location = create_test_location(&ctx.pool, "loc");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "group");
        let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

        let result = service.delete_producer(other.id, producer.id);
        assert!(matches!(result, Err(ServiceError::Forbidden)));
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_rotate_credentials_panics() {
        let (ctx, service) = setup_service();
        let user = create_test_user(&ctx.pool, "user");
        let _ = service.rotate_credentials(user.id, Uuid::new_v4());
    }
}

#[cfg(test)]
mod integration_test {
    use super::*;
    use crate::dao::group::PgGroupDao;
    use crate::dao::location::PgLocationDao;
    use crate::dao::producer::PgProducerDao;
    use crate::interface::types::location::NewLocation;
    use crate::interface::types::producer::NewProducer;
    use crate::test_utils::{create_test_group, create_test_location, create_test_user, setup};

    #[test]
    fn test_producer_lifecycle_flow() {
        let ctx = setup();
        let service =
            ProducerService::<PgProducerDao, PgLocationDao, PgGroupDao>::from(ctx.pool.clone());
        let user = create_test_user(&ctx.pool, "lifecycle");
        let location = create_test_location(&ctx.pool, "lifecycle");
        let group = create_test_group(&ctx.pool, user.id, location.id, "lifecycle");

        // 1. Create
        let new_producer = NewProducer {
            location: NewLocation {
                name: Some("Lifecycle Loc".to_string()),
                geo_lat: Some(10.0),
                geo_long: Some(20.0),
            },
            group_id: group.id,
            name: "Lifecycle Prod".to_string(),
            description: "Desc".to_string(),
            forwarder_id: None,
        };
        let created = service.create_producer(user.id, new_producer).unwrap();

        // 2. Get
        let fetched = service.get_producer(user.id, created.id).unwrap();
        assert_eq!(fetched.name, "Lifecycle Prod");

        // 3. Delete
        service.delete_producer(user.id, created.id).unwrap();

        // 4. Verify
        let result = service.get_producer(user.id, created.id);
        assert!(matches!(result, Err(ServiceError::NotFound)));
    }
}
