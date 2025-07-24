use craft::configuration::get_config;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let settings = get_config().expect("Failed to load configuration");

    let addr = std::net::SocketAddr::from((settings.app_settings.host, settings.app_settings.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    let db_url = settings.database.get_connection();
    let pool = PgPool::connect_lazy(&db_url).expect("Failed to connect to the database");

    craft::run(listener, pool).await;
}
