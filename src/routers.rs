mod health_check;
mod subscriptions;

use std::sync::Arc;

use axum::routing::{get, post};
use sqlx::{Pool, Postgres};
use tower_http::trace::TraceLayer;

use crate::email_client::EmailClient;

pub fn get_router(
    pool: Pool<Postgres>,
    email_client: EmailClient,
) -> axum::Router {
    // we can pass EmailClient directly through wit_state
    // we here we just want to demonstrate that with Arc, no string inside EmailClient will be cloned
    let email_client = Arc::new(email_client);

    axum::Router::new()
        .route("/health", get(health_check::health_check))
        .route("/subscriptions", post(subscriptions::subscript))
        .with_state(pool)
        .with_state(email_client)
        .layer(TraceLayer::new_for_http())
}
