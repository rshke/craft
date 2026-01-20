use std::sync::Arc;

use anyhow::Context;
use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    app_state::AppState,
    authentication::{Credentials, UserId},
    domain::{subscriber::SubscriberStatus, subscriber_email::SubscriberEmail},
    idempotency::{
        key::IdempotencyKey,
        persistence::{
            NextAction, get_saved_response, save_response, try_process,
        },
    },
    utils::AppError,
};

#[derive(Deserialize, Debug)]
pub struct Body {
    title: String,
    content: Content,
    idempotency_key: String,
}

#[derive(Deserialize, Debug)]
pub struct Content {
    text: String,
    html: String,
}

#[instrument(
    name = "Publish newsletter to confirmed users",
    skip(app_state, body)
)]
pub(crate) async fn publish_newsletter(
    State(app_state): State<Arc<AppState>>,
    Extension(user_id): Extension<UserId>,
    Json(body): Json<Body>,
) -> Result<Response, AppError> {
    let idempotency_key: IdempotencyKey =
        body.idempotency_key.try_into().map_err(AppError::E400)?;

    let tx =
        match try_process(&app_state.pool, &idempotency_key, user_id).await? {
            NextAction::StartProcessing(t) => t,
            NextAction::ReturnSavedResponse(saved_response) => {
                tracing::debug!(
                    "Found saved response for idempotency key: {}",
                    idempotency_key.as_ref()
                );
                return Ok(saved_response);
            }
        };

    let emails = get_confirmed_subscribers(&app_state.pool).await?;

    for email in emails {
        match email {
            Ok(email) => {
                app_state
                    .email_client
                    .send_email(
                        &email,
                        &body.title,
                        &body.content.text,
                        &body.content.html,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    // We record the error chain as a structured field
                    // on the log record.
                    error.cause_chain = ?error,
                    // Using `\` to split a long string literal over
                    // two lines, without creating a `\n` character.
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                );
            }
        }
    }

    let response = StatusCode::OK.into_response();
    let response = save_response(tx, idempotency_key, user_id, response)
        .await
        .map_err(AppError::E500)?;
    Ok(response)
}

#[deprecated(note = "Use session to manage authentication")]
#[allow(dead_code)]
fn basic_authentication(
    headers: &HeaderMap,
) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded_bytes = BASE64_STANDARD
        .decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .context("A username must be provided in 'Basic' auth.")?
        .to_string();
    let password = credentials
        .next()
        .context("A password must be provided in 'Basic' auth.")?
        .to_string();

    Ok(Credentials {
        username,
        password: SecretString::from(password),
    })
}

#[instrument(name = "get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<SubscriberEmail, anyhow::Error>>, anyhow::Error> {
    let records = sqlx::query!(
        r#"
         SELECT email
         FROM subscriptions
         WHERE status = $1
        "#,
        SubscriberStatus::Confirmed.to_string()
    )
    .fetch_all(pool)
    .await?;

    let emails = records
        .iter()
        .map(|r| match r.email.parse::<SubscriberEmail>() {
            Ok(email) => Ok(email),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(emails)
}
