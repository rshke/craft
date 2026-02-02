use std::sync::Arc;

use crate::{
    app_state::AppState,
    domain::{
        subscriber::{Subscriber, SubscriberStatus},
        subscriber_email::SubscriberEmail,
    },
    email_client::EmailClient,
    routers::error_chain_fmt,
};
use anyhow::Context;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use rand::distr::{Alphanumeric, SampleString};
use reqwest::Url;
use sqlx::{Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum SubscriptionError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl SubscriptionError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Debug for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for SubscriptionError {
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
        response
            .extensions_mut()
            .insert(Arc::new(anyhow::anyhow!(self)));

        response
    }
}

#[instrument(
    name = "Adding a new subscriber",
    skip(app_state, user),
    fields(
        request_id = %uuid::Uuid::new_v4(),
        subscriber_email = %user.email.as_ref(),
        subscriber_name = %user.name.as_ref()
    )
)]
pub(crate) async fn subscript(
    State(app_state): State<Arc<AppState>>,
    Json(user): Json<Subscriber>,
) -> Result<StatusCode, SubscriptionError> {
    let mut tx = app_state
        .pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;

    let subscriber_id = insert_user(&mut tx, &user)
        .await
        .context("Failed to insert new subscriber in the database.")?;

    let token = generate_token();

    store_token(&mut tx, subscriber_id, &token).await.context(
        "Failed to store the confirmation token for a new subscriber.",
    )?;

    tx.commit().await.context(
        "Failed to commit SQL transaction to store a new subscriber.",
    )?;

    send_confirmation_email(
        &app_state.email_client,
        &app_state.base_url,
        user.email,
        &token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(StatusCode::OK)
}

#[instrument(
    name = "Send confirmation email to subscriber",
    skip(email_client, to)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    base_url: &str,
    to: SubscriberEmail,
    token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = {
        let mut l = Url::parse(base_url).expect("Invalid base url");
        l.set_path("/subscriptions/confirm");
        l.query_pairs_mut().append_pair("token", token);

        l
    };

    let html_body = format!(
        "Welcome to our newsletter!<br />\
                Click <a href=\"{confirmation_link}\">here</a> to confirm your subscription."
    );
    let pain_text_body = format!(
        "Welcome to our newsletter!\nVisit {confirmation_link} to confirm your subscription."
    );

    email_client
        .send_email(&to, "Welcome", &pain_text_body, &html_body)
        .await
}

#[instrument(name = "Inserting a new user in the database", skip(tx, user))]
async fn insert_user(
    tx: &mut Transaction<'_, Postgres>,
    user: &Subscriber,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, name, email, subscribed_at, status)
        VALUES ($1, $2, $3, NOW(), $4)
        "#,
        id,
        user.name.as_ref(),
        user.email.as_ref(),
        SubscriberStatus::PendingConfirmation.to_string(),
    )
    .execute(&mut **tx) // WHY ????
    .await?;

    Ok(id)
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while \
            trying to store a subscription token."
        )
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[instrument(name = "Store subscription token in the database", skip(tx))]
async fn store_token(
    tx: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (subscription_token, subscription_id)
        VALUES ($1, $2)
        "#,
        token,
        subscriber_id,
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        // tracing::error!("Failed to execute query: {e:?}");

        StoreTokenError(e)
    })?;

    Ok(())
}

#[instrument(name = "Generate subscription token")]
fn generate_token() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 25)
}
