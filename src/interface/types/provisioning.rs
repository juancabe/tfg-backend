//! Provisioning-related API type definitions.
#![allow(missing_docs)]

use crate::helpers::validation::{
    DESCRIPTION_MAX_LEN, NAME_MAX_LEN, NAME_MIN_LEN, validate_firmware_name,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::location::NewLocation;

/// Request body for creating a pending producer provisioning.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Validate)]
pub struct CreatePendingProvisioningRequest {
    pub group_id: Uuid,
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub producer_name: String,
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub ble_name: String,
    #[validate(length(min = 1, max = DESCRIPTION_MAX_LEN))]
    pub description: String,
    pub location: NewLocation,
    pub forwarder_id: Option<Uuid>,
    pub forward_lora: Option<bool>,
    pub data_template_id: Option<Uuid>,
}

/// View of an available forwarder producer.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AvailableForwarderView {
    pub id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub lora_addr: i32,
    pub x25519_pub_key_bytes_base64: String,
}

/// Response listing available forwarders.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ListAvailableForwardersResponse {
    pub forwarders: Vec<AvailableForwarderView>,
}

/// WiFi provisioning bundle containing MQTT credentials for a device.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProvisioningWifiBundle {
    pub broker_host: String,
    pub broker_port: i16,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub publish_topic: Option<String>,
    pub subscribe_topic: String,
}

/// View of a pending provisioning registration.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PendingProvisioningView {
    pub id: Uuid,
    pub group_id: Uuid,
    pub producer_name: String,
    pub ble_name: String,
    pub description: String,
    pub data_template_id: Option<Uuid>,
    pub forwarder_id: Option<Uuid>,
    pub forward_lora: bool,
    pub status: i16,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Response for a successfully created pending provisioning.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreatePendingProvisioningResponse {
    pub pending: PendingProvisioningView,
    pub wifi_bundle: ProvisioningWifiBundle,
}

/// Request body for confirming a pending provisioning.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Validate)]
pub struct ConfirmPendingProvisioningRequest {
    #[validate(length(min = 16, max = 2048))]
    pub x25519_pub_key_bytes_base64: Option<String>,
    #[validate(range(min = 1, max = 65535))]
    pub lora_addr: Option<i32>,
}

/// Response for a confirmed provisioning.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConfirmPendingProvisioningResponse {
    pub pending_id: Uuid,
    pub producer_id: Uuid,
    pub status: i16,
    pub forwarder_id: Option<Uuid>,
}

/// Response for a cancelled provisioning.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CancelPendingProvisioningResponse {
    pub pending_id: Uuid,
    pub status: i16,
}

/// Response listing pending provisioning registrations.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ListPendingProvisioningResponse {
    pub pendings: Vec<PendingProvisioningView>,
}
