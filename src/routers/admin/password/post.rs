use std::sync::Arc;

use axum::{
    Extension,
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};

use crate::{
    app_state::AppState,
    authentication::{AuthError, Credentials, UserId, validate_credentials},
    routers::admin::dashboard::get_username,
    utils::AppError,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: SecretString,
    new_password: SecretString,
    new_password_check: SecretString,
}

const MIN_PASSWORD_LENGTH: usize = 12;

#[tracing::instrument(
    name = "Admin change password",
    skip(app_state, form),
    fields(
        user_id=tracing::field::Empty
    )
)]
pub async fn change_password(
    State(app_state): State<Arc<AppState>>,
    Extension(user_id): Extension<UserId>,
    axum::extract::Form(form): axum::extract::Form<FormData>,
) -> Result<Response, AppError> {
    let user_id = user_id.into_inner();

    if form.new_password.expose_secret()
        != form.new_password_check.expose_secret()
    {
        return Ok(Redirect::to("/admin/password").into_response());
    }
    if form.new_password.expose_secret().len() < MIN_PASSWORD_LENGTH {
        return Ok(Redirect::to("/admin/password").into_response());
    }

    let username = get_username(user_id, &app_state.pool)
        .await
        .map_err(AppError::E500)?;

    let credentials = Credentials {
        username,
        password: form.current_password,
    };

    if let Err(e) = validate_credentials(&app_state.pool, credentials).await {
        match e {
            AuthError::InvalidCredentials(e) => {
                tracing::info!("Invalid credentials: {:?}", e);
                return Ok(Redirect::to("/admin/password").into_response());
            }
            AuthError::UnexpectedError(_) => {
                return Err(AppError::E500(e.into()));
            }
        }
    };

    crate::authentication::change_password(
        user_id,
        form.new_password,
        &app_state.pool,
    )
    .await
    .map_err(AppError::E500)?;

    Ok(StatusCode::OK.into_response())
}
