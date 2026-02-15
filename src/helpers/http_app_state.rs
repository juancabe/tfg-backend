//! AppState, will mostly be used by the `http` interface

use crate::dao;

#[derive(Clone)]
pub struct AppState {
    pub pool: dao::DbPool,
}
