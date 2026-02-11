pub mod auth {
    use serde::{Deserialize, Serialize};
    #[derive(Serialize, Deserialize, Debug)]
    pub struct EmailPass {
        pub email: String,
        pub hashed_pass: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum AuthMethod {
        EmailPass(EmailPass),
    }
}

pub mod state {
    use crate::dao;

    #[derive(Clone)]
    pub struct AppState {
        pub pool: dao::DbPool,
    }
}

pub mod extractor {
    use axum::{
        extract::FromRequestParts,
        http::{StatusCode, header},
    };

    use crate::{
        dao::identity::PgIdentityDao,
        helpers::state::AppState,
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
