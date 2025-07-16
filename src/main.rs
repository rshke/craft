use craft::configuration::get_config;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = get_config().expect("Failed to load configuration");

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.application_port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    let db_url = config.database.get_connection_without_database();
    let pool = PgPool::connect(&db_url)
        .await
        .expect("Failed to connect to the database");

    craft::run(listener, pool).await;
}
