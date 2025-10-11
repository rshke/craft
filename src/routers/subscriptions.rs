use std::sync::Arc;

use crate::{
    app_state::AppState,
    domain::{
        subscriber::{Subscriber, SubscriberStatus},
        subscriber_email::SubscriberEmail,
    },
    email_client::EmailClient,
};
use axum::{Json, extract::State, http::StatusCode};
use rand::distr::{Alphanumeric, SampleString};
use reqwest::Url;
use sqlx::{Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

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
) -> StatusCode {
    let mut tx = match app_state.pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    let subscriber_id = match insert_user(&mut tx, &user).await {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    let token = generate_token();
    
    if store_token(&mut tx, subscriber_id, &token)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    if tx.commit().await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    if send_confirmation_email(
        &app_state.email_client,
        &app_state.base_url,
        user.email,
        &token,
    )
    .await
    .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    StatusCode::OK
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
        .send_email(to, "Welcome", &pain_text_body, &html_body)
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    Ok(id)
}

#[instrument(name = "Store subscription token in the database", skip(tx))]
async fn store_token(
    tx: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    token: &str,
) -> Result<(), sqlx::Error> {
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
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    Ok(())
}

#[instrument(name = "Generate subscription token")]
fn generate_token() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 25)
}
