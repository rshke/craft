use tokio::net::TcpListener;

use craft::run;

#[tokio::test]
async fn health_check_works() {
    let settings = craft::configuration::get_config()
        .expect("Failed to load configuration");

    let addr = std::net::SocketAddr::from((
        settings.app_settings.host,
        settings.app_settings.port,
    ));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let addr = listener.local_addr().unwrap();

    spawn_server(listener, settings.database.get_connection()).await;

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
