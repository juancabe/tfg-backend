pub mod jwt_extractor {
    use axum::{
        extract::FromRequestParts,
        http::{StatusCode, header},
    };

    use crate::{
        dao::identity::PgIdentityDao,
        helpers::http_app_state::AppState,
        service::auth::{AuthService, JWT},
    };

    impl FromRequestParts<AppState> for JWT {
        type Rejection = StatusCode;

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            AppState { pool }: &AppState,
        ) -> Result<Self, Self::Rejection> {
            let auth = parts
                .headers
                .get(header::AUTHORIZATION)
                .ok_or(StatusCode::UNAUTHORIZED)?;

            let encoded = auth
                .to_str()
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .strip_prefix("Bearer ")
                .ok_or(StatusCode::UNAUTHORIZED)?;

            let service = AuthService::new(PgIdentityDao::new(pool.clone()));
            service
                .validate_session(encoded)
                .map_err(|_| StatusCode::UNAUTHORIZED)
        }
    }
}
