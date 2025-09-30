use std::collections::HashMap;

use craft::startup::Application;
use craft::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};

use craft::configuration::DBSettings;

pub struct TestApp {
    pub address: String,
    pub pool: PgPool,
}

impl TestApp {
    pub async fn post_subscriptions(
        &self,
        body: &HashMap<&'static str, &'static str>,
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

static INIT_SUBSCRIBER: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "debug".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(
            subscriber_name,
            default_filter_level,
            std::io::stdout,
        );
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(
            subscriber_name,
            default_filter_level,
            std::io::sink,
        );
        init_subscriber(subscriber);
    }
});

pub async fn spawn_app() -> TestApp {
    Lazy::force(&INIT_SUBSCRIBER);

    let mut app_config = craft::configuration::get_config()
        .expect("Failed to load configuration");

    app_config.database.database_name = format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "_")
    );

    let pool = configure_database(&app_config.database).await;

    let app = Application::build(app_config)
        .await
        .expect("Failed to build application");

    let app_url = format!("http://127.0.0.1:{}", app.port());

    tokio::spawn(app.run_until_stop());

    TestApp {
        address: app_url,
        pool,
    }
}

async fn configure_database(configuration: &DBSettings) -> PgPool {
    let url = configuration.get_connection_without_database();
    let mut db_connection =
        PgConnection::connect(&url).await.unwrap_or_else(|_| {
            panic!("Failed to connect to postgres server: {}", url)
        });
    db_connection
        .execute(
            format!("CREATE DATABASE {};", configuration.database_name)
                .as_str(),
        )
        .await
        .expect("Failed to create database");
    db_connection
        .close()
        .await
        .expect("Failed to close connection");

    let pool = sqlx::PgPool::connect(configuration.get_connection().as_str())
        .await
        .expect("Failed to connect to the database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}
