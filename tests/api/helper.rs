use std::collections::HashMap;

use craft::startup::Application;
use craft::telemetry::{get_subscriber, init_subscriber};
use fake::Fake;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name;
use linkify::LinkFinder;
use once_cell::sync::Lazy;
use serde_json::Value;
use sqlx::{Connection, Executor, PgConnection, PgPool};

use craft::configuration::DBSettings;
use reqwest::Url;
use wiremock::{MockServer, Request};

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub pool: PgPool,
    pub email_server: MockServer,
}

pub struct ConfirmationLinks {
    pub html: Url,
    pub pain_text: Url,
}

impl TestApp {
    pub async fn post_subscriptions(
        &self,
        body: &HashMap<String, String>,
    ) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: &Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/newsletters", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn extract_links(&self, request: &Request) -> ConfirmationLinks {
        let body: serde_json::Value =
            serde_json::from_slice(&request.body).unwrap();

        let get_link = |s: &str| {
            let finder = LinkFinder::new();
            let links: Vec<_> = finder
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(1, links.len());

            links[0].as_str().to_owned()
        };

        let html_link = {
            let raw_l = get_link(body["HtmlBody"].as_str().unwrap());
            let mut l = Url::parse(&raw_l).unwrap();
            l.set_port(Some(self.port)).unwrap();

            l
        };

        let text_link = {
            let raw_l = get_link(body["TextBody"].as_str().unwrap());
            let mut l = Url::parse(&raw_l).unwrap();
            l.set_port(Some(self.port)).unwrap();

            l
        };

        ConfirmationLinks {
            html: html_link,
            pain_text: text_link,
        }
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

    let email_server = MockServer::start().await;

    let app_config = {
        let mut c = craft::configuration::get_config()
            .expect("Failed to load configuration");

        c.database.database_name = format!(
            "test_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "_")
        );
        c.email_client.base_url = email_server.uri();

        c
    };

    let pool = configure_database(&app_config.database).await;

    let app = Application::build(app_config)
        .await
        .expect("Failed to build application");

    let app_port = app.port();
    let app_url = format!("http://127.0.0.1:{}", app_port);

    tokio::spawn(app.run_until_stop());

    TestApp {
        address: app_url,
        port: app_port,
        pool,
        email_server,
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

pub fn valid_subscriber() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let name: String = name::en::Name().fake();
    let email: String = SafeEmail().fake();
    map.insert("name".to_string(), name);
    map.insert("email".to_string(), email);

    map
}
