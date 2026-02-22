use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

// Import your controllers and types here. 
// Adjust these paths based on your actual module structure!
use crate::interface::http::{auth_controller, producer_controller};
use crate::interface::types::{group::Group, producer::{NewProducer, Producer}};
use crate::helpers::users_auth::AuthMethod;

/// Security modifier to add Bearer token support to Swagger UI
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

/// Central OpenAPI registry
#[derive(OpenApi)]
#[openapi(
    // 1. Register all your endpoint functions here
    paths(
        auth_controller::post_login,
        auth_controller::post_register,
        auth_controller::post_refresh,
        producer_controller::get_producers,
        producer_controller::get_producer_by_id,
        producer_controller::post_producer,
    ),
    // 2. Register all structs/enums used in requests or responses
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
            AuthMethod, // Ensure AuthMethod derives ToSchema
            
            // Producer Schemas
            producer_controller::GetProducersResponse,
            producer_controller::GetProducerResponse,
            producer_controller::PostProducerBody,
            producer_controller::PostProducerResponse,
            Group,       // Ensure Group derives ToSchema
            Producer,    // Ensure Producer derives ToSchema
            NewProducer, // Ensure NewProducer derives ToSchema
        )
    ),
    // 3. Apply the security modifier
    modifiers(&SecurityAddon),
    // 4. Organize endpoints into categories
    tags(
        (name = "auth", description = "Authentication and authorization endpoints"),
        (name = "producers", description = "Producer management endpoints")
    )
)]
pub struct ApiDoc;
