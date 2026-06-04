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
        // Extract Authorization header or query parameter "token"
        let token = if let Some(auth_header) = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        {
            if !auth_header.starts_with("Bearer ") {
                return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header format"));
            }
            auth_header[7..].to_string()
        } else {
            let query = parts.uri.query().unwrap_or("");
            let token_param = query
                .split('&')
                .find(|part| part.starts_with("token="))
                .map(|part| part[6..].to_string());
            
            token_param.ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header or token query parameter"))?
        };

        let claims = verify_token(&token, &state.config.jwt_secret)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

        Ok(claims)
    }
}

