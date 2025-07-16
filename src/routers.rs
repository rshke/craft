mod health_check;
mod subscriptions;

use axum::routing::{get, post};
use sqlx::{Pool, Postgres};

pub fn get_router(pool: Pool<Postgres>) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health_check::health_check))
        .route("/subscriptions", post(subscriptions::subscript))
        .with_state(pool)
}
