use std::{ops::Deref, sync::Arc};

use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
    response::{IntoResponse, Response},
    RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use oidc_jwt_validator::Validator;
use serde::Deserialize;

/// Claims<T> can be used to require OpenID Connect authorization
/// The supplied struct can be used to fetch possible relevant OpenID Connect claims
/// 
/// Usage: `async fn some_axum_handler(claims: Claims<MyClaims>)`
#[derive(Debug)]
pub(super) struct Claims<T: for<'de> Deserialize<'de>>(T);

impl<T: for<'de> Deserialize<'de>> Deref for Claims<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(super) enum AuthError {
    InvalidToken(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            AuthError::InvalidToken(msg) => Response::builder()
                .status(401)
                .header(
                    "WWW-Authenticate",
                    format!(
                        r#"Bearer realm="ssh-casign" error="invalid_token" error_description="{}""#,
                        msg
                    ),
                )
                .body(axum::body::Body::default())
                .expect("http invalid_token response"),
        }
    }
}

#[axum::async_trait]
impl<S, T: for<'de> Deserialize<'de>> FromRequestParts<S> for Claims<T>
where
    Arc<Validator>: FromRef<S>,
    S: Send + Sync,
{
    // If anything goes wrong or no session is found, redirect to the auth page
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|err| AuthError::InvalidToken(err.to_string()))?;

        // Validate token
        let oidc_validator = Arc::<Validator>::from_ref(state);
        let token_data = oidc_validator
            .validate::<T>(bearer.token())
            .await
            .map_err(|err| AuthError::InvalidToken(err.to_string()))?;

        // Return claims
        Ok(Claims(token_data.claims))
    }
}