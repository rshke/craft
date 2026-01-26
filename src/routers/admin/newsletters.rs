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
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;

use crate::{
    app_state::AppState,
    authentication::{Credentials, UserId},
    domain::{subscriber::SubscriberStatus, subscriber_email::SubscriberEmail},
    idempotency::{
        key::IdempotencyKey,
        persistence::{NextAction, save_response, try_process},
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
    skip_all,
    fields(
        // reborrow user_id to avoid moving it
        user_id = %&*user_id,
    )
)]
pub(crate) async fn publish_newsletter(
    State(app_state): State<Arc<AppState>>,
    Extension(user_id): Extension<UserId>,
    Json(body): Json<Body>,
) -> Result<Response, AppError> {
    let idempotency_key: IdempotencyKey =
        body.idempotency_key.try_into().map_err(AppError::E400)?;

    let mut tx =
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

    let newsletter_issue_id = save_newsletter_issue(
        &mut tx,
        &body.title,
        &body.content.text,
        &body.content.html,
    )
    .await
    .context("Failed to store newsletter issue detailes")
    .map_err(AppError::E500)?;
    enqueue_delivery_tasks(&mut tx, newsletter_issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(AppError::E500)?;

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

#[allow(dead_code)]
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

#[instrument(skip_all)]
async fn save_newsletter_issue(
    tx: &mut Transaction<'static, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<uuid::Uuid, anyhow::Error> {
    let newsletter_issue_id = uuid::Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, NOW())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(&mut **tx)
    .await?;

    Ok(newsletter_issue_id)
}

#[instrument(skip_all)]
async fn enqueue_delivery_tasks(
    tx: &mut Transaction<'static, Postgres>,
    newsletter_issue_id: uuid::Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id, subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        "#,
        newsletter_issue_id
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
