//! Producer-related API type definitions.
#![allow(missing_docs)]

use crate::helpers::validation::{
    DESCRIPTION_MAX_LEN, NAME_MAX_LEN, NAME_MIN_LEN, validate_firmware_name,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    interface::types::location::{Location, NewLocation},
    model::models,
};

/// Request body for creating a new producer.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema, Validate)]
pub struct NewProducer {
    pub location: NewLocation,
    pub group_id: Uuid,
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub name: String,
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub ble_name: String,
    #[validate(length(min = 1, max = DESCRIPTION_MAX_LEN))]
    pub description: String,
    pub forwarder_id: Option<Uuid>,
    pub lora_addr: Option<i32>,
    pub forward_lora: Option<bool>,
}

/// Request body for updating an existing producer.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema, Validate)]
pub struct UpdateProducer {
    #[validate(length(min = 1, max = DESCRIPTION_MAX_LEN))]
    pub description: Option<String>,
}

/// API representation of a producer with its location.
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct Producer {
    pub id: Uuid,
    pub location: Location,
    pub group_id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub forwarder_id: Option<Uuid>,
    pub lora_addr: Option<i32>,
    pub forward_lora: bool,
}

impl Producer {
    /// Constructs a [`Producer`] from a database producer and location pair.
    pub fn from_producer_location(producer: models::Producer, location: models::Location) -> Self {
        let models::Producer {
            id,
            location_id: _,
            group_id,
            name,
            ble_name,
            description,
            last_seen,
            created_at,
            updated_at,
            forwarder_id,
            lora_addr,
            forward_lora,
        } = producer;

        let models::Location {
            id: _,
            name: loc_name,
            geo_lat,
            geo_long,
        } = location;

        Self {
            id,
            location: Location {
                name: loc_name,
                geo_lat,
                geo_long,
            },
            group_id,
            name,
            ble_name,
            description,
            last_seen,
            created_at,
            updated_at,
            forwarder_id,
            lora_addr,
            forward_lora,
        }
    }
}
