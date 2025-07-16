mod health_check;
mod subscriptions;

use sqlx::{Pool, Postgres};

pub fn get_router(pool: Pool<Postgres>) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(health_check::health_check))
        .route(
            "/subscriptions",
            axum::routing::post(subscriptions::subscript),
        )
        .with_state(pool)
}
