use std::sync::Arc;

use anyhow::Context;
use axum::{
    Extension,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::{app_state::AppState, authentication::UserId};

#[instrument(
    name = "login admin dashboard page"
    skip(user_id, app_state)
)]
pub async fn admin_dashboard(
    Extension(user_id): Extension<UserId>,
    State(app_state): State<Arc<AppState>>,
) -> Response {
    let user_id = user_id.into_inner();

    let username = match get_username(user_id, &app_state.pool).await {
        Ok(username) => username,
        Err(e) => {
            tracing::info!("Failed to get username {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Faied to get username {e:?}"),
            )
                .into_response();
        }
    };

    let html_content = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
<title>Admin dashboard</title>
</head>
<body>
<p>Welcome {username}!</p>
</body>
</html>"#
    );
    axum::response::Html(html_content).into_response()
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(
    user_id: Uuid,
    pool: &PgPool,
) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;

    Ok(row.username)
}
