mod health_check;
mod subscriptions;
mod subscriptions_confirm;

use std::sync::Arc;

use axum::routing::{get, post};
use sqlx::{Pool, Postgres};
use tower_http::trace::TraceLayer;

use crate::{app_state::AppState, email_client::EmailClient};

pub fn get_router(
    pool: Pool<Postgres>,
    email_client: EmailClient,
    base_url: String,
) -> axum::Router {
    // we can pass EmailClient directly through wit_state
    // we here we just want to demonstrate that with Arc, no string inside EmailClient will be cloned
    let email_client = email_client;

    let app_state = Arc::new(AppState {
        pool,
        email_client,
        base_url,
    });

    axum::Router::new()
        .route("/health", get(health_check::health_check))
        .route("/subscriptions", post(subscriptions::subscript))
        .route(
            "/subscriptions/confirm",
            get(subscriptions_confirm::confirm),
        )
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
}
