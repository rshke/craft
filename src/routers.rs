mod health_check;
mod home;
mod login;
mod newsletters;
mod subscriptions;
mod subscriptions_confirm;

use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use axum::routing::{get, post};
use sqlx::{Pool, Postgres};
use tower_http::trace::TraceLayer;

use crate::{app_state::AppState, email_client::EmailClient};

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

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
        .route("/newsletters", post(newsletters::publish_newsletter))
        .route("/login", get(login::login_form))
        .route("/login", post(login::login))
        .route("/", get(home::home))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(from_fn(log_app_errors))
}

async fn log_app_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    if let Some(err) = response.extensions().get::<Arc<anyhow::Error>>() {
        tracing::error!(?err, "an unexpected error occurred inside a handler");
    }
    response
}
