use std::sync::Arc;

use anyhow::Context;
use axum::{Json, extract::State, response::IntoResponse};
use reqwest::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    app_state::AppState,
    domain::{subscriber::SubscriberStatus, subscriber_email::SubscriberEmail},
    routers::error_chain_fmt,
};

#[derive(Deserialize, Debug)]
pub struct Body {
    title: String,
    content: Content,
}

#[derive(Deserialize, Debug)]
pub struct Content {
    text: String,
    html: String,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for PublishError {
    fn into_response(self) -> axum::response::Response {
        #[derive(serde::Serialize)]
        struct ErrorResponse {
            message: String,
            details: String,
        }

        let message = format!("{self}");
        let details = format!("{:?}", self);
        let body = Json(ErrorResponse { message, details });

        let status_code = StatusCode::INTERNAL_SERVER_ERROR;
        let mut response = (status_code, body).into_response();
        response
            .extensions_mut()
            .insert(Arc::new(anyhow::anyhow!(self)));

        response
    }
}

#[instrument(name = "Publish newsletter to confirmed users", skip(app_state))]
pub(crate) async fn publish_newsletter(
    State(app_state): State<Arc<AppState>>,
    Json(body): Json<Body>,
) -> Result<StatusCode, PublishError> {
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

    Ok(StatusCode::OK)
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
