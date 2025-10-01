use craft::configuration::get_config;
use craft::startup::Application;
use craft::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() {
    let subscriber =
        get_subscriber("craft".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    dotenvy::dotenv().ok();

    let settings = get_config().expect("Failed to load configuration");

    let app = Application::build(settings)
        .await
        .expect("Failed to build application");
    app.run_until_stop().await;
}
