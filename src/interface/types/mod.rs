use thiserror::Error;

pub mod group;
pub mod location;
pub mod producer;
pub mod provisioning;

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Location error")]
    LocationError(#[from] location::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}
