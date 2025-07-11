use tokio::net::TcpListener;

async fn health_check() -> &'static str {
    "OK"
}

pub async fn run(listener: TcpListener) {
    let app = axum::Router::new().route("/health", axum::routing::get(health_check));
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
