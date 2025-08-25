use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;

pub mod configuration;
pub mod domain;
mod routers;
pub mod telemetry;

pub async fn run(listener: TcpListener, pool: Pool<Postgres>) {
    let app = routers::get_router(pool);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
