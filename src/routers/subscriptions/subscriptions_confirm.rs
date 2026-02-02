use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::domain::subscriber::SubscriberStatus;

#[derive(Deserialize, Debug)]
pub struct Params {
    token: String,
}

#[instrument(
    name = "Confirm a pending confirmed subscription",
    skip(app_state)
)]
pub(crate) async fn confirm(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<Params>,
) -> StatusCode {
    let subscriber_id =
        match match_subscription(&app_state.pool, &params.token).await {
            Ok(uuid) => match uuid {
                Some(id) => id,
                None => return StatusCode::UNAUTHORIZED,
            },
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
        };

    if confirm_subscription(&app_state.pool, subscriber_id)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}

#[instrument(name = "Match subscription token", skip(pool, token))]
async fn match_subscription(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
         SELECT subscription_id
         FROM subscription_tokens
         WHERE subscription_token = $1
        "#,
        token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    Ok(result.map(|r| r.subscription_id))
}

#[instrument(
    name = "Change subscription status to confirm in db",
    skip(pool, subscriber_id)
)]
async fn confirm_subscription(
    pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions
        SET status = $1
        WHERE id = $2
        "#,
        SubscriberStatus::Confirmed.to_string(),
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    Ok(())
}
