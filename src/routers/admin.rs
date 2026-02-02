mod dashboard;
use dashboard::*;

mod password;
use password::*;

mod logout;
use logout::*;

mod newsletters;
use newsletters::*;

use crate::app_state::AppState;
use axum::routing::{get, post};
use std::sync::Arc;

pub fn router() -> axum::routing::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/dashboard", get(admin_dashboard))
        .route("/password", get(change_password_form))
        .route("/password", post(change_password))
        .route("/logout", post(logout))
        .route("/newsletters", post(publish_newsletter))
}
