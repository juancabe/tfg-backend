use backend::dao::group::{GroupDao, PgGroupDao, GroupRole};
use backend::dao::producer::{PgProducerDao, ProducerDao};
use backend::dao::template::{PgTemplateDao, TemplateDao};
use backend::dao::telemetry::{PgTelemetryDao, TelemetryDao};
use backend::model::models::{GroupUpdate, NewPacketTemplate, NewTemplateField, NewData};

mod common;

#[test]
fn test_group_lifecycle() {
    let ctx = common::setup();
    let dao = PgGroupDao::new(ctx.pool.clone());

    let user = common::create_test_user(&ctx.pool, "1");
    let loc = common::create_test_location(&ctx.pool, "1");
    let group = common::create_test_group(&ctx.pool, user.id, loc.id, "1");

    // Check permissions
    assert_eq!(dao.check_permission(user.id, group.id).unwrap(), GroupRole::Owner);

    // Update group
    let updated = dao.update_group(group.id, GroupUpdate {
        name: Some("updated_name".to_string()),
        description: None,
        location_id: None,
        modification: None,
        visibility: None,
    }).unwrap();

    assert_eq!(updated.name, "updated_name");

    // Add member
    let user2 = common::create_test_user(&ctx.pool, "2");
    dao.add_member(group.id, user2.id).unwrap();
    assert_eq!(dao.check_permission(user2.id, group.id).unwrap(), GroupRole::Member);

    // Remove member
    dao.remove_member(group.id, user2.id).unwrap();
    assert_eq!(dao.check_permission(user2.id, group.id).unwrap(), GroupRole::None);
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

    let tmpl = dao.create_template(NewPacketTemplate {
        creator_user_id: Some(user.id),
        name: "test_tmpl".to_string(),
    }).unwrap();

    let _field = dao.add_field(NewTemplateField {
        template_id: tmpl.id,
        json_key: "temp".to_string(),
        name: "Temperature".to_string(),
        repr_enum_value: 1,
        unit_enum_value: 1,
        unit_enum_other_payload: None,
    }).unwrap();

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

    let tmpl = tmpl_dao.create_template(NewPacketTemplate {
        creator_user_id: Some(user.id),
        name: "telemetry_tmpl".to_string(),
    }).unwrap();

    let data_id = t_dao.ingest_packet(NewData {
        producer_id: producer.id,
        template_id: tmpl.id,
        payload: serde_json::json!({"temp": 25.5}),
    }).unwrap();

    let latest = t_dao.get_latest_packet(producer.id).unwrap().unwrap();
    assert_eq!(latest.id, data_id);
    assert_eq!(latest.payload["temp"], 25.5);
}
