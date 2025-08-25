use crate::domain::SubscriberName;
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::instrument;

#[derive(Deserialize, Debug)]
pub struct User {
    pub name: SubscriberName,
    pub email: String,
}

#[instrument(
    name = "Adding a new subscriber",
    skip(pool, user),
    fields(
        request_id = %uuid::Uuid::new_v4(),
        subscriber_email = %user.email,
        subscriber_name = %user.name.as_ref()
    )
)]
pub(crate) async fn subscript(
    State(pool): State<PgPool>,
    Json(user): Json<User>,
) -> StatusCode {
    match insert_user(&pool, &user).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[instrument(name = "Inserting a new user in the database", skip(pool, user))]
async fn insert_user(pool: &PgPool, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, name, email, subscribed_at)
        VALUES (gen_random_uuid(), $1, $2, NOW())
        "#,
        user.name.as_ref(),
        user.email
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    Ok(())
}
