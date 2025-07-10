use tokio::time::{sleep, Duration};

use craft::get_server;

#[tokio::test]
async fn health_check_works() {
    spawn_server().await;

    sleep(Duration::from_secs(3)).await; // Wait for the server to start

    let response = reqwest::get("http://127.0.0.1:3000/health")
        .await
        .expect("Failed to send request");

    println!("Response: {:?}", response);
    assert!(response.status().is_success(), "Health check failed");
}

async fn spawn_server() {
    let (app, listener) = get_server().await;
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("Failed to start server");
    });
}
