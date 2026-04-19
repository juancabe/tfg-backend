use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::location::NewLocation;

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreatePendingProvisioningRequest {
    pub group_id: Uuid,
    pub producer_name: String,
    pub ble_name: String,
    pub description: String,
    pub location: NewLocation,
    pub forwarder_id: Option<Uuid>,
    pub data_template_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProvisioningWifiBundle {
    pub broker_host: String,
    pub broker_port: i16,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub publish_topic: Option<String>,
    pub subscribe_topic: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PendingProvisioningView {
    pub id: Uuid,
    pub group_id: Uuid,
    pub producer_name: String,
    pub ble_name: String,
    pub description: String,
    pub data_template_id: Option<Uuid>,
    pub forwarder_id: Option<Uuid>,
    pub status: i16,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreatePendingProvisioningResponse {
    pub pending: PendingProvisioningView,
    pub wifi_bundle: ProvisioningWifiBundle,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConfirmPendingProvisioningRequest {
    pub x25519_pub_key_bytes_base64: Option<String>,
    pub lora_addr: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConfirmPendingProvisioningResponse {
    pub pending_id: Uuid,
    pub producer_id: Uuid,
    pub status: i16,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CancelPendingProvisioningResponse {
    pub pending_id: Uuid,
    pub status: i16,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ListPendingProvisioningResponse {
    pub pendings: Vec<PendingProvisioningView>,
}
