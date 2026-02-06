use backend::dao::command::{CommandDao, PgCommandDao};
use backend::dao::group::{GroupDao, GroupRole, PgGroupDao};
use backend::dao::identity::{IdentityDao, PgIdentityDao};
use backend::dao::producer::{PgProducerDao, ProducerDao};
use backend::dao::telemetry::{PgTelemetryDao, TelemetryDao};
use backend::dao::template::{PgTemplateDao, TemplateDao};
use backend::model::models::{
    GroupUpdate, NewCommand, NewData, NewEmailAuthMethod, NewPacketTemplate, NewRefreshToken,
    NewTemplateField, NewUser, RefreshToken,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

mod common;

#[test]
fn test_group_lifecycle() {
    let ctx = common::setup();
    let dao = PgGroupDao::new(ctx.pool.clone());

    let user = common::create_test_user(&ctx.pool, "1");
    let loc = common::create_test_location(&ctx.pool, "1");
    let group = common::create_test_group(&ctx.pool, user.id, loc.id, "1");

    // Check permissions
    assert_eq!(
        dao.check_permission(user.id, group.id).unwrap(),
        GroupRole::Owner
    );

    // Update group
    let updated = dao
        .update_group(
            group.id,
            GroupUpdate {
                name: Some("updated_name".to_string()),
                description: None,
                location_id: None,
                modification: None,
                visibility: None,
            },
        )
        .unwrap();

    assert_eq!(updated.name, "updated_name");

    // Add member
    let user2 = common::create_test_user(&ctx.pool, "2");
    dao.add_member(group.id, user2.id).unwrap();
    assert_eq!(
        dao.check_permission(user2.id, group.id).unwrap(),
        GroupRole::Member
    );

    // Remove member
    dao.remove_member(group.id, user2.id).unwrap();
    assert_eq!(
        dao.check_permission(user2.id, group.id).unwrap(),
        GroupRole::None
    );
}

#[test]
fn test_producer_lifecycle() {
    let ctx = common::setup();
    let dao = PgProducerDao::new(ctx.pool.clone());

    let user = common::create_test_user(&ctx.pool, "p1");
    let loc = common::create_test_location(&ctx.pool, "p1");
    let group = common::create_test_group(&ctx.pool, user.id, loc.id, "p1");

    let producer = common::create_test_producer(&ctx.pool, group.id, loc.id, "p1");

    let fetched = dao.get_by_id(producer.id).unwrap().unwrap();
    assert_eq!(fetched.name, producer.name);

    dao.update_last_seen(producer.id).unwrap();
    let updated = dao.get_by_id(producer.id).unwrap().unwrap();
    assert!(updated.last_seen > producer.last_seen);
}

#[test]
fn test_template_lifecycle() {
    let ctx = common::setup();
    let dao = PgTemplateDao::new(ctx.pool.clone());
    let user = common::create_test_user(&ctx.pool, "t1");

    let tmpl = dao
        .create_template(NewPacketTemplate {
            creator_user_id: Some(user.id),
            name: "test_tmpl".to_string(),
        })
        .unwrap();

    let _field = dao
        .add_field(NewTemplateField {
            template_id: tmpl.id,
            json_key: "temp".to_string(),
            name: "Temperature".to_string(),
            repr_enum_value: 1,
            unit_enum_value: 1,
            unit_enum_other_payload: None,
        })
        .unwrap();

    let structure = dao.get_template_structure(tmpl.id).unwrap();
    assert_eq!(structure.template.name, "test_tmpl");
    assert_eq!(structure.fields.len(), 1);
    assert_eq!(structure.fields[0].json_key, "temp");
}

#[test]
fn test_telemetry_ingest() {
    let ctx = common::setup();
    let t_dao = PgTelemetryDao::new(ctx.pool.clone());
    let tmpl_dao = PgTemplateDao::new(ctx.pool.clone());

    let user = common::create_test_user(&ctx.pool, "tel1");
    let loc = common::create_test_location(&ctx.pool, "tel1");
    let group = common::create_test_group(&ctx.pool, user.id, loc.id, "tel1");
    let producer = common::create_test_producer(&ctx.pool, group.id, loc.id, "tel1");

    let tmpl = tmpl_dao
        .create_template(NewPacketTemplate {
            creator_user_id: Some(user.id),
            name: "telemetry_tmpl".to_string(),
        })
        .unwrap();

    let data_id = t_dao
        .ingest_packet(NewData {
            producer_id: producer.id,
            template_id: tmpl.id,
            payload: serde_json::json!({"temp": 25.5}),
        })
        .unwrap();

    let latest = t_dao.get_latest_packet(producer.id).unwrap().unwrap();
    assert_eq!(latest.id, data_id);
    assert_eq!(latest.payload["temp"], 25.5);
}

#[test]
fn test_command_lifecycle() {
    let ctx = common::setup();
    let dao = PgCommandDao::new(ctx.pool.clone());

    let user = common::create_test_user(&ctx.pool, "c1");
    let loc = common::create_test_location(&ctx.pool, "c1");
    let group = common::create_test_group(&ctx.pool, user.id, loc.id, "c1");
    let producer = common::create_test_producer(&ctx.pool, group.id, loc.id, "c1");

    let cmd = dao
        .queue_command(NewCommand {
            target_producer_id: producer.id,
            status: 1, // Sent
            kind: 1,   // Reboot
            payload: serde_json::json!({"delay": 5}),
        })
        .unwrap();

    let pending = dao.fetch_pending_commands(producer.id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, cmd.id);

    let history = dao.get_command_history(producer.id).unwrap();
    assert_eq!(history.len(), 1);
}

#[test]
fn test_identity_lifecycle() {
    let ctx = common::setup();
    let dao = PgIdentityDao::new(ctx.pool.clone());

    let user = dao
        .create_user(NewUser {
            login: "test_identity",
        })
        .unwrap();
    let email = "email@email.com";
    let hashed_pass = "this_is_a_hash_believe_me";

    let fetched_by_login = dao.find_by_login("test_identity").unwrap().unwrap();
    assert_eq!(fetched_by_login.id, user.id);

    let fetched_by_id = dao.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(fetched_by_id.login, "test_identity");

    let auth = dao
        .add_email_auth_method(NewEmailAuthMethod {
            user_id: user.id,
            email,
            hashed_pass,
        })
        .unwrap();
    assert_eq!(auth.user_id, user.id);

    let token_version_n = dao
        .rotate_refresh_token(NewRefreshToken {
            user_id: user.id,
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        })
        .unwrap();

    assert!(dao.verify_token(&user.id, token_version_n).unwrap());
}

#[test]
fn test_identity_token_rotation() {
    use backend::schema::refresh_tokens::dsl::*;
    let ctx = common::setup();
    let dao = PgIdentityDao::new(ctx.pool.clone());
    let user = common::create_test_user(&ctx.pool, "rotation_test");

    // Create a first token
    let first_token_version_n = dao
        .rotate_refresh_token(NewRefreshToken {
            user_id: user.id,
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        })
        .unwrap();
    assert!(dao.verify_token(&user.id, first_token_version_n).unwrap());

    // Rotate the token
    let second_token_version_n = dao
        .rotate_refresh_token(NewRefreshToken {
            user_id: user.id,
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        })
        .unwrap();

    // 3. Verify the second token is valid and the first is not
    assert!(dao.verify_token(&user.id, second_token_version_n).unwrap());
    assert!(!dao.verify_token(&user.id, first_token_version_n).unwrap());

    // Check the `claimed` flag in the database for the first token
    let mut conn = ctx.pool.get().unwrap();
    let first_token_db = refresh_tokens
        .filter(version_n.eq(first_token_version_n))
        .filter(user_id.eq(user.id))
        .get_result::<RefreshToken>(&mut conn)
        .unwrap();
    assert!(first_token_db.claimed);
}
