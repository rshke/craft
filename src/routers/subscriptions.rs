mod post;
mod subscriptions_confirm;

use crate::app_state::AppState;
use axum::routing::{get, post};
use std::sync::Arc;

pub fn router() -> axum::routing::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/subscriptions", post(post::subscript))
        .route(
            "/subscriptions/confirm",
            get(subscriptions_confirm::confirm),
        )
}
