use thiserror::Error;

pub mod location;
pub mod producer;

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Location error")]
    LocationError(#[from] location::Error),
}
