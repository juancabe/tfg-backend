use serde::{Deserialize, Serialize};
use validator::Validate;
use crate::helpers::validation::{DESCRIPTION_MAX_LEN, NAME_MAX_LEN, NAME_MIN_LEN, validate_firmware_name};
use uuid::Uuid;

use crate::{interface::types::location::NewLocation, model::models};

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct Group {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub location_id: Uuid,
    pub name: String,
    pub description: String,
    pub is_public: bool,
}

impl From<models::Group> for Group {
    fn from(value: models::Group) -> Self {
        let models::Group {
            id,
            owner_user_id,
            location_id,
            name,
            description,
            is_public,
        } = value;
        Self {
            id,
            owner_user_id,
            location_id,
            name,
            description,
            is_public,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema, Validate)]
pub struct NewGroup {
    pub location: NewLocation,
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub name: String,
    #[validate(length(min = 1, max = DESCRIPTION_MAX_LEN))]
    pub description: String,
    pub is_public: bool,
}

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema, Validate)]
pub struct UpdateGroup {
    #[validate(length(min = NAME_MIN_LEN, max = NAME_MAX_LEN), custom(function = "validate_firmware_name"))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = DESCRIPTION_MAX_LEN))]
    pub description: Option<String>,
    pub is_public: Option<bool>,
}
