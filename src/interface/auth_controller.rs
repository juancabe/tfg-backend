use axum::{Json, extract::rejection::JsonRejection};
use serde::Deserialize;

use crate::{
    dao::identity::{IdentityDao, PgIdentityDao},
    interface::ApiError,
};

pub struct AuthController {}

#[derive(Deserialize)]
pub struct RefreshPayload {
    refresh_token: String,
}

impl AuthController {
    pub fn post_refresh(
        payload: Result<Json<RefreshPayload>, JsonRejection>,
    ) -> Result<(), ApiError> {
        todo!()
        // let payload = payload?;
        // let service = PgIdentityDao::new(pool);
        // service.rotate_refresh_token(token);
        // todo!()
    }
}
