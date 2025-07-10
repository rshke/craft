use tokio::time::{sleep, Duration};

use craft::get_server;

#[tokio::test]
async fn health_check_works() {
    let addr = spawn_server().await;

    sleep(Duration::from_secs(3)).await; // Wait for the server to start

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("Failed to send request");

    println!("Response: {response:?}");
    assert!(response.status().is_success(), "Health check failed");
}

async fn spawn_server() -> std::net::SocketAddr {
    let (app, listener) = get_server().await;
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    tokio::spawn(server.into_future());
    // tokio::spawn(async move {
    //     axum::serve(listener, app.into_make_service())
    //         .await
    //         .expect("Failed to start server");
    // });

    addr
}
