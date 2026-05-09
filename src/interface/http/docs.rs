use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::interface::http::{
    auth_controller, group_controller, producer_controller, provisioning_controller,
    template_data_controller,
};
use crate::interface::types::{
    group::{Group, NewGroup, UpdateGroup},
    location::NewLocation,
    producer::{NewProducer, Producer, UpdateProducer},
    provisioning::{
        AvailableForwarderView, CancelPendingProvisioningResponse,
        ConfirmPendingProvisioningRequest, ConfirmPendingProvisioningResponse,
        CreatePendingProvisioningRequest, CreatePendingProvisioningResponse,
        ListAvailableForwardersResponse, ListPendingProvisioningResponse, PendingProvisioningView,
        ProvisioningWifiBundle,
    },
    template_data::{
        DataSampleView, PacketTemplateView, ProducerDataQuery, ProducerDataView, TemplateFieldView,
    },
};

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
        auth_controller::post_change_password,
        auth_controller::post_password_reset_request,
        auth_controller::post_password_reset,
        producer_controller::get_producers,
        producer_controller::get_producer_by_id,
        producer_controller::post_producer,
        producer_controller::patch_producer_by_id,
        producer_controller::delete_producer_by_id,
        group_controller::get_groups,
        group_controller::get_group_by_id,
        group_controller::post_group,
        group_controller::patch_group_by_id,
        group_controller::delete_group_by_id,
        provisioning_controller::post_pending,
        provisioning_controller::get_pending,
        provisioning_controller::get_available_forwarders,
        provisioning_controller::post_pending_confirm,
        provisioning_controller::post_pending_cancel,
        template_data_controller::get_templates,
        template_data_controller::get_producer_data,
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
            auth_controller::ChangePasswordPayload,
            auth_controller::ChangePasswordResponse,
            auth_controller::PasswordResetRequestPayload,
            auth_controller::PasswordResetPayload,
            crate::interface::types::auth::AuthMethod,

            producer_controller::GetProducersResponse,
            producer_controller::GetProducerResponse,
            producer_controller::PostProducerBody,
            producer_controller::PostProducerResponse,
            producer_controller::PatchProducerBody,
            producer_controller::PatchProducerResponse,

            group_controller::GetGroupsResponse,
            group_controller::GetGroupResponse,
            group_controller::PostGroupBody,
            group_controller::PostGroupResponse,
            group_controller::PatchGroupBody,
            group_controller::PatchGroupResponse,

            Group,
            NewGroup,
            UpdateGroup,
            NewLocation,
            Producer,
            NewProducer,
            UpdateProducer,

            CreatePendingProvisioningRequest,
            CreatePendingProvisioningResponse,
            PendingProvisioningView,
            ProvisioningWifiBundle,
            AvailableForwarderView,
            ListAvailableForwardersResponse,
            ConfirmPendingProvisioningRequest,
            ConfirmPendingProvisioningResponse,
            CancelPendingProvisioningResponse,
            ListPendingProvisioningResponse,

            template_data_controller::GetTemplatesResponse,
            template_data_controller::GetProducerDataResponse,
            PacketTemplateView,
            TemplateFieldView,
            DataSampleView,
            ProducerDataQuery,
            ProducerDataView,
            crate::interface::http::ErrorResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication and authorization endpoints"),
        (name = "producers", description = "Producer management endpoints"),
        (name = "groups", description = "Group management endpoints"),
        (name = "provisioning", description = "Device provisioning and pending registrations"),
        (name = "templates", description = "Packet templates and producer data endpoints")
    )
)]
pub struct ApiDoc;
