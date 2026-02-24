use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{interface::types::location::NewLocation, model::models};

#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct NewProducer {
    pub location: NewLocation,
    pub group_id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub forwarder_id: Option<Uuid>,
}

// TODO: Don't wrap it, complete new type
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct Producer {
    pub id: Uuid,
    pub location_id: Uuid,
    pub group_id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub forwarder_id: Option<Uuid>,
}

impl From<models::Producer> for Producer {
    fn from(value: models::Producer) -> Self {
        let models::Producer {
            id,
            location_id,
            group_id,
            name,
            ble_name,
            description,
            last_seen,
            created_at,
            updated_at,
            forwarder_id,
        } = value;

        Self {
            id,
            location_id,
            group_id,
            name,
            ble_name,
            description,
            last_seen,
            created_at,
            updated_at,
            forwarder_id,
        }
    }
}
