use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::provisioning::{
            CancelPendingProvisioningResponse, ConfirmPendingProvisioningRequest,
            ConfirmPendingProvisioningResponse, CreatePendingProvisioningRequest,
            CreatePendingProvisioningResponse, ListAvailableForwardersResponse,
            ListPendingProvisioningResponse,
        },
    },
    service::{auth::JWT, provisioning::ProvisioningService},
};

#[utoipa::path(
    post,
    path = "/provisioning/pending",
    tag = "provisioning",
    request_body = CreatePendingProvisioningRequest,
    responses((status = 201, description = "Created pending provisioning", body = CreatePendingProvisioningResponse)),
    security(("bearer_auth" = []))
)]
pub async fn post_pending(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Json(body): Json<CreatePendingProvisioningRequest>,
) -> Result<(StatusCode, Json<CreatePendingProvisioningResponse>), HttpApiError> {
    let service: ProvisioningService<_, _, _, _> = pool.into();
    let created = service.create_pending(jwt.user_id, body)?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    get,
    path = "/provisioning/pending",
    tag = "provisioning",
    responses((status = 200, description = "List current user's pending provisioning requests", body = ListPendingProvisioningResponse)),
    security(("bearer_auth" = []))
)]
pub async fn get_pending(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
) -> Result<Json<ListPendingProvisioningResponse>, HttpApiError> {
    let service: ProvisioningService<_, _, _, _> = pool.into();
    let out = service.list_pending_for_owner(jwt.user_id)?;
    Ok(Json(out))
}

#[utoipa::path(
    get,
    path = "/provisioning/groups/{group_id}/available-forwarders",
    tag = "provisioning",
    params(("group_id" = Uuid, Path, description = "Group id")),
    responses((status = 200, description = "List available forwarders for group", body = ListAvailableForwardersResponse)),
    security(("bearer_auth" = []))
)]
pub async fn get_available_forwarders(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<ListAvailableForwardersResponse>, HttpApiError> {
    let service: ProvisioningService<_, _, _, _> = pool.into();
    let out = service.list_available_forwarders(jwt.user_id, group_id)?;
    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/provisioning/pending/{id}/confirm",
    tag = "provisioning",
    request_body = ConfirmPendingProvisioningRequest,
    params(("id" = Uuid, Path, description = "Pending provisioning id")),
    responses((status = 200, description = "Confirmed pending provisioning", body = ConfirmPendingProvisioningResponse)),
    security(("bearer_auth" = []))
)]
pub async fn post_pending_confirm(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ConfirmPendingProvisioningRequest>,
) -> Result<Json<ConfirmPendingProvisioningResponse>, HttpApiError> {
    let service: ProvisioningService<_, _, _, _> = pool.into();
    let out = service.confirm_pending(jwt.user_id, id, body)?;
    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/provisioning/pending/{id}/cancel",
    tag = "provisioning",
    params(("id" = Uuid, Path, description = "Pending provisioning id")),
    responses((status = 200, description = "Cancelled pending provisioning", body = CancelPendingProvisioningResponse)),
    security(("bearer_auth" = []))
)]
pub async fn post_pending_cancel(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CancelPendingProvisioningResponse>, HttpApiError> {
    let service: ProvisioningService<_, _, _, _> = pool.into();
    let out = service.cancel_pending(jwt.user_id, id)?;
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    use diesel::prelude::*;

    use crate::{
        helpers::crypto_password,
        helpers::data_templates::RANDOMFLOAT_TEMPLATE_ID,
        interface::types::{
            location::NewLocation,
            provisioning::{CreatePendingProvisioningRequest, PendingProvisioningView},
        },
        model::{
            models::{
                InternetAuth, MqttBrokerAuth, NewInternetAuth, NewPacketTemplate, PacketTemplate,
                PendingProducerRegistration, PendingProducerRegistrationStatus, Producer,
            },
            schema::{
                internet_auths, mqtt_broker_auths, packet_templates,
                pending_producer_registrations, producers,
            },
        },
        service::auth::JWTKind,
        test_utils::{
            create_test_group, create_test_location, create_test_producer, create_test_user, setup,
        },
    };

    fn mock_jwt(user_id: Uuid, login: String) -> JWT {
        let (iat, exp): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
            JWTKind::Session.get_iat_exp();
        JWT::new(
            1,
            login,
            user_id,
            JWTKind::Session,
            iat.timestamp(),
            exp.timestamp(),
        )
    }

    fn state(pool: crate::dao::DbPool) -> State<AppState> {
        State(AppState { pool })
    }

    fn mk_pending_req(
        group_id: Uuid,
        template_id: Option<Uuid>,
    ) -> CreatePendingProvisioningRequest {
        CreatePendingProvisioningRequest {
            group_id,
            producer_name: "producer01".to_string(),
            ble_name: "ble-producer01".to_string(),
            description: "test producer".to_string(),
            location: NewLocation {
                name: Some("loc-a".to_string()),
                geo_lat: Some(10.0),
                geo_long: Some(20.0),
            },
            forwarder_id: None,
            forward_lora: None,
            data_template_id: template_id,
        }
    }

    fn make_forwarder_available(
        pool: &crate::dao::DbPool,
        producer_id: Uuid,
        lora: i32,
        key: &str,
    ) {
        let mut conn = pool.get().expect("db conn");

        diesel::update(producers::table.find(producer_id))
            .set((
                producers::forward_lora.eq(true),
                producers::lora_addr.eq(Some(lora)),
            ))
            .execute(&mut conn)
            .expect("mark producer as forwarder");

        diesel::insert_into(internet_auths::table)
            .values(NewInternetAuth {
                producer_id,
                x25519_pub_key_bytes_base64: key.to_string(),
            })
            .execute(&mut conn)
            .expect("insert internet auth");
    }

    #[tokio::test]
    async fn test_default_randomfloat_template_is_seeded() {
        let ctx = setup();
        let mut conn = ctx.pool.get().expect("db conn");

        let seeded = packet_templates::table
            .find(Uuid::parse_str(RANDOMFLOAT_TEMPLATE_ID).expect("valid uuid"))
            .select(PacketTemplate::as_select())
            .get_result::<PacketTemplate>(&mut conn)
            .expect("seeded randomfloat template should exist");

        assert_eq!(seeded.name, "randomfloat");
    }

    #[tokio::test]
    async fn test_create_pending_with_seeded_randomfloat_template() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_seeded_tmpl");
        let loc = create_test_location(&ctx.pool, "prov_seeded_tmpl");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_seeded_tmpl");

        let template_id = Uuid::parse_str(RANDOMFLOAT_TEMPLATE_ID).expect("valid uuid");
        let created = post_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, Some(template_id))),
        )
        .await
        .expect("create pending")
        .1
        .0;

        let publish_topic = created
            .wifi_bundle
            .publish_topic
            .clone()
            .expect("publish topic should exist");
        assert!(publish_topic.contains(RANDOMFLOAT_TEMPLATE_ID));
    }

    #[tokio::test]
    async fn test_create_pending_rejects_unknown_template() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_bad_tmpl");
        let loc = create_test_location(&ctx.pool, "prov_bad_tmpl");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_bad_tmpl");

        let res = post_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, Some(Uuid::new_v4()))),
        )
        .await;

        assert!(res.is_err());
        let response = res.unwrap_err().into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    fn find_pending_by_id(pool: &crate::dao::DbPool, id: Uuid) -> PendingProducerRegistration {
        let mut conn = pool.get().expect("db conn");
        pending_producer_registrations::table
            .find(id)
            .select(PendingProducerRegistration::as_select())
            .get_result(&mut conn)
            .expect("pending row")
    }

    #[tokio::test]
    async fn test_create_and_list_pending_success_no_template() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_create");
        let loc = create_test_location(&ctx.pool, "prov_create");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_create");

        let created = post_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, None)),
        )
        .await
        .expect("create pending")
        .1
        .0;

        assert_eq!(
            created.pending.status,
            PendingProducerRegistrationStatus::Pending as i16
        );
        assert!(created.wifi_bundle.publish_topic.is_none());
        assert!(!created.wifi_bundle.mqtt_password.is_empty());

        let pending = find_pending_by_id(&ctx.pool, created.pending.id);
        assert_eq!(
            pending.status,
            PendingProducerRegistrationStatus::Pending as i16
        );
        assert_ne!(
            pending.mqtt_password_hash,
            created.wifi_bundle.mqtt_password
        );
        assert!(crypto_password::verify_password(
            &pending.mqtt_password_hash,
            &created.wifi_bundle.mqtt_password,
        ));

        let listed = get_pending(mock_jwt(user.id, user.login), state(ctx.pool.clone()))
            .await
            .expect("list pending")
            .0
            .pendings;

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.pending.id);
    }

    #[tokio::test]
    async fn test_confirm_pending_creates_records_and_marks_confirmed() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_confirm");
        let loc = create_test_location(&ctx.pool, "prov_confirm");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_confirm");

        let template = {
            let mut conn = ctx.pool.get().expect("db conn");
            diesel::insert_into(packet_templates::table)
                .values(NewPacketTemplate {
                    creator_user_id: Some(user.id),
                    name: "temp-a".to_string(),
                })
                .returning(PacketTemplate::as_returning())
                .get_result::<PacketTemplate>(&mut conn)
                .expect("create template")
        };

        let created = post_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, Some(template.id))),
        )
        .await
        .expect("create pending")
        .1
        .0;

        let publish_topic = created
            .wifi_bundle
            .publish_topic
            .clone()
            .expect("publish topic should exist for template-based producer");
        assert!(publish_topic.contains(&template.id.to_string()));
        assert!(publish_topic.contains(&group.name));

        let confirmed = post_pending_confirm(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Path(created.pending.id),
            Json(ConfirmPendingProvisioningRequest {
                x25519_pub_key_bytes_base64: Some("aW50ZXJuZXQtcHViLWtleQ==".to_string()),
                lora_addr: Some(321),
            }),
        )
        .await
        .expect("confirm pending")
        .0;

        assert_eq!(
            confirmed.status,
            PendingProducerRegistrationStatus::Confirmed as i16
        );

        let pending = find_pending_by_id(&ctx.pool, created.pending.id);
        assert_eq!(pending.id, confirmed.pending_id);
        assert_eq!(pending.producer_id, Some(confirmed.producer_id));
        assert_eq!(
            pending.status,
            PendingProducerRegistrationStatus::Confirmed as i16
        );
        assert_eq!(pending.lora_addr, Some(321));
        assert_eq!(
            pending.x25519_pub_key_bytes_base64.as_deref(),
            Some("aW50ZXJuZXQtcHViLWtleQ==")
        );

        let mut conn = ctx.pool.get().expect("db conn");
        let producer = producers::table
            .find(confirmed.producer_id)
            .select(Producer::as_select())
            .get_result::<Producer>(&mut conn)
            .expect("producer row");
        assert_eq!(producer.name, "producer01");

        let mqtt_auth = mqtt_broker_auths::table
            .find(confirmed.producer_id)
            .select(MqttBrokerAuth::as_select())
            .get_result::<MqttBrokerAuth>(&mut conn)
            .expect("mqtt auth row");
        assert_eq!(mqtt_auth.username, created.wifi_bundle.mqtt_username);
        assert!(crypto_password::verify_password(
            &mqtt_auth.password_hash,
            &created.wifi_bundle.mqtt_password,
        ));

        let internet_auth = internet_auths::table
            .find(confirmed.producer_id)
            .select(InternetAuth::as_select())
            .get_result::<InternetAuth>(&mut conn)
            .expect("internet auth row");
        assert_eq!(
            internet_auth.x25519_pub_key_bytes_base64,
            "aW50ZXJuZXQtcHViLWtleQ=="
        );
    }

    #[tokio::test]
    async fn test_get_available_forwarders_lists_only_eligible_forwarders() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "prov_list_fwd");
        let loc = create_test_location(&ctx.pool, "prov_list_fwd");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "prov_list_fwd");
        let other_group = create_test_group(&ctx.pool, owner.id, loc.id, "prov_list_fwd_other");

        let eligible = create_test_producer(&ctx.pool, group.id, loc.id, "eligible");
        make_forwarder_available(&ctx.pool, eligible.id, 1001, "a2V5LWVsaWdpYmxl");

        let no_auth = create_test_producer(&ctx.pool, group.id, loc.id, "no_auth");
        {
            let mut conn = ctx.pool.get().expect("db conn");
            diesel::update(producers::table.find(no_auth.id))
                .set((
                    producers::forward_lora.eq(true),
                    producers::lora_addr.eq(Some(1002)),
                ))
                .execute(&mut conn)
                .expect("mark no_auth producer");
        }

        let no_lora = create_test_producer(&ctx.pool, group.id, loc.id, "no_lora");
        {
            let mut conn = ctx.pool.get().expect("db conn");
            diesel::update(producers::table.find(no_lora.id))
                .set(producers::forward_lora.eq(true))
                .execute(&mut conn)
                .expect("mark no_lora producer");
            diesel::insert_into(internet_auths::table)
                .values(NewInternetAuth {
                    producer_id: no_lora.id,
                    x25519_pub_key_bytes_base64: "a2V5LW5vLWxvcmE=".to_string(),
                })
                .execute(&mut conn)
                .expect("insert no_lora internet auth");
        }

        let other_group_eligible =
            create_test_producer(&ctx.pool, other_group.id, loc.id, "other_group");
        make_forwarder_available(
            &ctx.pool,
            other_group_eligible.id,
            1003,
            "a2V5LW90aGVyLWdyb3Vw",
        );

        let out = get_available_forwarders(
            mock_jwt(owner.id, owner.login),
            state(ctx.pool.clone()),
            Path(group.id),
        )
        .await
        .expect("list available forwarders")
        .0;

        assert_eq!(out.forwarders.len(), 1);
        assert_eq!(out.forwarders[0].id, eligible.id);
        assert_eq!(out.forwarders[0].lora_addr, 1001);
        assert_eq!(
            out.forwarders[0].x25519_pub_key_bytes_base64,
            "a2V5LWVsaWdpYmxl"
        );
    }

    #[tokio::test]
    async fn test_get_available_forwarders_forbidden_for_non_owner() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "prov_list_fwd_owner");
        let other = create_test_user(&ctx.pool, "prov_list_fwd_other");
        let loc = create_test_location(&ctx.pool, "prov_list_fwd_forbidden");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "prov_list_fwd_forbidden");

        let res = get_available_forwarders(
            mock_jwt(other.id, other.login),
            state(ctx.pool.clone()),
            Path(group.id),
        )
        .await;

        assert!(res.is_err());
        let response = res.unwrap_err().into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_create_pending_rejects_forward_lora_with_forwarder_id() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_forward_conflict");
        let loc = create_test_location(&ctx.pool, "prov_forward_conflict");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_forward_conflict");
        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "prov_forward_conflict");

        let mut req = mk_pending_req(group.id, None);
        req.forward_lora = Some(true);
        req.forwarder_id = Some(forwarder.id);

        let res = post_pending(
            mock_jwt(user.id, user.login),
            state(ctx.pool.clone()),
            Json(req),
        )
        .await;

        assert!(res.is_err());
        let response = res.unwrap_err().into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_pending_rejects_unavailable_forwarder() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_bad_forwarder");
        let loc = create_test_location(&ctx.pool, "prov_bad_forwarder");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_bad_forwarder");
        let not_available = create_test_producer(&ctx.pool, group.id, loc.id, "prov_bad_forwarder");

        let mut req = mk_pending_req(group.id, None);
        req.forwarder_id = Some(not_available.id);

        let res = post_pending(
            mock_jwt(user.id, user.login),
            state(ctx.pool.clone()),
            Json(req),
        )
        .await;

        assert!(res.is_err());
        let response = res.unwrap_err().into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_pending_accepts_available_forwarder() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_ok_forwarder");
        let loc = create_test_location(&ctx.pool, "prov_ok_forwarder");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_ok_forwarder");
        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "prov_ok_forwarder");
        make_forwarder_available(&ctx.pool, forwarder.id, 2020, "a2V5LW9rLWZvcndhcmRlcg==");

        let mut req = mk_pending_req(group.id, None);
        req.forwarder_id = Some(forwarder.id);

        let created = post_pending(
            mock_jwt(user.id, user.login),
            state(ctx.pool.clone()),
            Json(req),
        )
        .await
        .expect("create pending with forwarder")
        .1
        .0;

        assert_eq!(created.pending.forwarder_id, Some(forwarder.id));
        assert!(!created.pending.forward_lora);

        let pending = find_pending_by_id(&ctx.pool, created.pending.id);
        assert_eq!(pending.forwarder_id, Some(forwarder.id));
        assert!(!pending.forward_lora);
    }

    #[tokio::test]
    async fn test_cancel_pending_removes_from_list_and_blocks_confirm() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "prov_cancel");
        let loc = create_test_location(&ctx.pool, "prov_cancel");
        let group = create_test_group(&ctx.pool, user.id, loc.id, "prov_cancel");

        let created = post_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, None)),
        )
        .await
        .expect("create pending")
        .1
        .0;

        let cancelled = post_pending_cancel(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
            Path(created.pending.id),
        )
        .await
        .expect("cancel pending")
        .0;
        assert_eq!(
            cancelled.status,
            PendingProducerRegistrationStatus::Cancelled as i16
        );

        let listed = get_pending(
            mock_jwt(user.id, user.login.clone()),
            state(ctx.pool.clone()),
        )
        .await
        .expect("list pending")
        .0
        .pendings;
        assert!(listed.is_empty());

        let confirm_attempt = post_pending_confirm(
            mock_jwt(user.id, user.login),
            state(ctx.pool.clone()),
            Path(created.pending.id),
            Json(ConfirmPendingProvisioningRequest {
                x25519_pub_key_bytes_base64: None,
                lora_addr: None,
            }),
        )
        .await;
        assert!(confirm_attempt.is_err());
    }

    #[tokio::test]
    async fn test_other_user_cannot_manage_pending() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "prov_owner");
        let other = create_test_user(&ctx.pool, "prov_other");
        let loc = create_test_location(&ctx.pool, "prov_owner");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "prov_owner");

        let created = post_pending(
            mock_jwt(owner.id, owner.login.clone()),
            state(ctx.pool.clone()),
            Json(mk_pending_req(group.id, None)),
        )
        .await
        .expect("create pending")
        .1
        .0;

        let cancel_as_other = post_pending_cancel(
            mock_jwt(other.id, other.login.clone()),
            state(ctx.pool.clone()),
            Path(created.pending.id),
        )
        .await;
        assert!(cancel_as_other.is_err());

        let confirm_as_other = post_pending_confirm(
            mock_jwt(other.id, other.login),
            state(ctx.pool.clone()),
            Path(created.pending.id),
            Json(ConfirmPendingProvisioningRequest {
                x25519_pub_key_bytes_base64: None,
                lora_addr: None,
            }),
        )
        .await;
        assert!(confirm_as_other.is_err());

        let listed = get_pending(mock_jwt(owner.id, owner.login), state(ctx.pool.clone()))
            .await
            .expect("list pending")
            .0
            .pendings;
        assert_eq!(listed.len(), 1);
    }

    #[allow(dead_code)]
    fn _assert_pending_view(_v: PendingProvisioningView) {}
}
