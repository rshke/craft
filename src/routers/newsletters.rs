use std::sync::Arc;

use anyhow::Context;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use base64::prelude::*;
use secrecy::SecretString;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    app_state::AppState,
    authentication::{AuthError, Credentials, valid_credentials},
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
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::AuthError(_) => StatusCode::UNAUTHORIZED,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
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

        let status_code = self.status_code();
        let mut response = (status_code, body).into_response();

        match self {
            Self::AuthError(_) => {
                response.headers_mut().insert(
                    "WWW-Authenticate",
                    HeaderValue::from_static(r#"Basic realm="publish""#),
                );
            }
            Self::UnexpectedError(_) => (),
        }

        response
            .extensions_mut()
            .insert(Arc::new(anyhow::anyhow!(self)));

        response
    }
}

#[instrument(
    name = "Publish newsletter to confirmed users",
    skip(app_state, headers, body),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub(crate) async fn publish_newsletter(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Body>,
) -> Result<StatusCode, PublishError> {
    let _credentials =
        basic_authentication(&headers).map_err(PublishError::AuthError)?;
    tracing::Span::current()
        .record("username", tracing::field::display(&_credentials.username));

    let user_id = valid_credentials(&app_state.pool, _credentials)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => {
                PublishError::AuthError(e.into())
            }
            AuthError::UnexpectedError(_) => {
                PublishError::UnexpectedError(e.into())
            }
        })?;
    tracing::Span::current()
        .record("user_id", tracing::field::display(&user_id));

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
