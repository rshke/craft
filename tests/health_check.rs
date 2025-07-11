use tokio::net::TcpListener;

use craft::run;

#[tokio::test]
async fn health_check_works() {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let addr = listener.local_addr().unwrap();

    spawn_server(listener).await;

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success(), "Health check failed");
}

async fn spawn_server(listener: TcpListener) {
    tokio::spawn(run(listener));
}
