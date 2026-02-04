use crate::dao::command::CommandDao;
use crate::dao::group::GroupDao;
use crate::dao::producer::ProducerDao;
use crate::model::models::{Command, NewCommand};
use crate::service::ServiceError;
use crate::service::producer_access::ProducerAccessService;
use uuid::Uuid;

pub struct CommandService<C: CommandDao, P: ProducerDao, G: GroupDao> {
    _dao: C,
    _access_service: ProducerAccessService<P, G>,
}

impl<C: CommandDao, P: ProducerDao, G: GroupDao> CommandService<C, P, G> {
    pub fn new(dao: C, producer_dao: P, group_dao: G) -> Self {
        Self {
            _dao: dao,
            _access_service: ProducerAccessService::new(producer_dao, group_dao),
        }
    }

    pub fn send_command(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
        _cmd: NewCommand,
    ) -> Result<Command, ServiceError> {
        todo!()
    }

    pub fn get_history(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<Vec<Command>, ServiceError> {
        todo!()
    }
}
