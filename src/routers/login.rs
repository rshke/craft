mod get;
mod post;

use crate::app_state::AppState;
use axum::routing::{get, post};
use std::sync::Arc;

pub fn router() -> axum::routing::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/login", get(get::login_form))
        .route("/login", post(post::login))
}
