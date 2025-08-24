mod health_check;
mod subscriptions;

use axum::routing::{get, post};
use sqlx::{Pool, Postgres};
use tower_http::trace::TraceLayer;

pub fn get_router(pool: Pool<Postgres>) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health_check::health_check))
        .route("/subscriptions", post(subscriptions::subscript))
        .with_state(pool)
        .layer(TraceLayer::new_for_http())
}
