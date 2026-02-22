use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::models;

// TODO: Don't wrap it, complete new type
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
