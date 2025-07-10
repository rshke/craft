pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn get_server() -> (axum::Router, tokio::net::TcpListener) {
    let app = axum::Router::new().route("/health", axum::routing::get(health_check));

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    (app, listener)
}
