use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::interface::{http::{auth_controller, group_controller, producer_controller}};
use crate::interface::types::{group::{Group, NewGroup}, producer::{NewProducer, Producer}};
use crate::helpers::users_auth::AuthMethod;

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        auth_controller::post_login,
        auth_controller::post_register,
        auth_controller::post_refresh,
        producer_controller::get_producers,
        producer_controller::get_producer_by_id,
        producer_controller::post_producer,
        group_controller::get_groups,
        group_controller::get_group_by_id,
        group_controller::post_group,
    ),
    components(
        schemas(
            // Auth Schemas
            auth_controller::RefreshPayload,
            auth_controller::RefreshSessionResponse,
            auth_controller::RegisterPayload,
            auth_controller::RegisterResponse,
            auth_controller::LoginPayload,
            auth_controller::LoginResponse,
            auth_controller::CorrectLoginResponse,
            AuthMethod,
            
            producer_controller::GetProducersResponse,
            producer_controller::GetProducerResponse,
            producer_controller::PostProducerBody,
            producer_controller::PostProducerResponse,

            group_controller::GetGroupsResponse,
            group_controller::GetGroupResponse,
            group_controller::PostGroupBody,
            group_controller::PostGroupResponse,
            
            Group,
            NewGroup,
            Producer,
            NewProducer,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication and authorization endpoints"),
        (name = "producers", description = "Producer management endpoints"),
        (name = "groups", description = "Group management endpoints")
    )
)]
pub struct ApiDoc;
