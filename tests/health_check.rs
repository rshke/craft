use tokio::net::TcpListener;

use craft::run;

#[tokio::test]
async fn health_check_works() {
    let config = craft::configuration::get_config().expect("Failed to load configuration");

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.application_port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let addr = listener.local_addr().unwrap();

    spawn_server(listener, config.database.get_connection()).await;

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("Failed to send request");

    assert!(
        response.status().is_success(),
        "Health check failed with status: {}",
        response.status()
    );
}

async fn spawn_server(listener: TcpListener, db_url: String) {
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("Failed to connect to the database");
    tokio::spawn(run(listener, pool));
}
