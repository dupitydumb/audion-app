use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    async_trait,
};
use crate::state::AppState;
use crate::auth::{verify_token, Claims};

#[async_trait]
impl FromRequestParts<AppState> for Claims {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        if !auth_header.starts_with("Bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header format"));
        }

        let token = &auth_header[7..];
        let claims = verify_token(token, &state.config.jwt_secret)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

        Ok(claims)
    }
}

