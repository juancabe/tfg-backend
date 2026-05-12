use serde::{Deserialize, Serialize};
use thiserror::Error;
use validator::Validate;

use crate::{interface::types::TypeError, model::models};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Only provided one component of geo location")]
    InvalidPartialLocation,
}

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct Location {
    pub name: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_long: Option<f64>,
}

#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema, Validate)]
pub struct NewLocation {
    pub name: Option<String>,
    #[validate(range(min = -90.0, max = 90.0))]
    pub geo_lat: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0))]
    pub geo_long: Option<f64>,
}

impl TryFrom<NewLocation> for models::NewLocation {
    type Error = TypeError;

    fn try_from(value: NewLocation) -> Result<Self, Self::Error> {
        value
            .validate()
            .map_err(|e| TypeError::Validation(e.to_string()))?;
        match (value.geo_lat, value.geo_long) {
            (None, None) | (Some(_), Some(_)) => Ok(Self {
                name: value.name,
                geo_lat: value.geo_lat,
                geo_long: value.geo_long,
            }),
            _ => Err(Error::InvalidPartialLocation)?,
        }
    }
}
