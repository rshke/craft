use std::sync::Arc;

use axum::{
    extract::State,
    response::{self, IntoResponse},
};
use reqwest::StatusCode;
use secrecy::SecretString;
use tracing::instrument;

use crate::{
    app_state::AppState,
    authentication::{AuthError, valid_credentials},
    routers::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: SecretString,
}

#[instrument(
    name = "User login",
    skip(app_state, form),
    fields(
        username=tracing::field::Empty,
        user_id=tracing::field::Empty
    )
)]
pub async fn login(
    State(app_state): State<Arc<AppState>>,
    axum::extract::Form(form): axum::extract::Form<LoginForm>,
) -> Result<response::Response, LoginError> {
    let _credentials = crate::authentication::Credentials {
        username: form.username,
        password: form.password,
    };

    tracing::Span::current()
        .record("username", tracing::field::display(&_credentials.username));

    let user_id = valid_credentials(&app_state.pool, _credentials)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => {
                LoginError::UnexpectedError(e.into())
            }
        })?;

    tracing::Span::current()
        .record("user_id", tracing::field::display(&user_id));

    Ok(response::Redirect::to("/").into_response())
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}
impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for LoginError {
    fn into_response(self) -> response::Response {
        let mut response = match self {
            Self::AuthError(_) => StatusCode::UNAUTHORIZED.into_response(),
            Self::UnexpectedError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
        response
            .extensions_mut()
            .insert(Arc::new(anyhow::anyhow!(self)));

        response
    }
}
