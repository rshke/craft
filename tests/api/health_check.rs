use crate::helper::spawn_app;

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;

    let response = reqwest::get(format!("{}/health", app.address))
        .await
        .expect("Failed to send request");

    assert!(
        response.status().is_success(),
        "Health check failed with status: {}",
        response.status()
    );
}
