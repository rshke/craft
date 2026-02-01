use argon2::Argon2;
use argon2::password_hash::PasswordHasher;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use craft::background_workers::idempotency_expire_wroker::try_clean_expired_idempotency;
use craft::background_workers::issue_delivery_worker::{
    ExecutionOutput, try_execute_task,
};
use craft::configuration::Settings;
use craft::email_client::EmailClient;
use craft::startup::Application;
use craft::telemetry::{get_subscriber, init_subscriber};
use fake::Fake;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name;
use linkify::LinkFinder;
use once_cell::sync::Lazy;
use reqwest::Response;
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::collections::HashMap;
use std::time::Duration;

use craft::configuration::DBSettings;
use reqwest::Url;
use uuid::Uuid;
use wiremock::{MockServer, Request};

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    fn generate() -> Self {
        TestUser {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        sqlx::query!(
            r#"
            INSERT INTO users (user_id, username, password_hash)
            VALUES ($1, $2, $3)
            "#,
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to create test users.");
    }
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub pool: PgPool,
    pub email_server: MockServer,
    pub test_user: TestUser,
    api_client: reqwest::Client,
    email_client: EmailClient,
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
        self.api_client
            .post(format!("{}/subscriptions", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: &Value) -> reqwest::Response {
        self.api_client
            .post(format!("{}/admin/newsletters", &self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_login(&self, body: &Value) -> reqwest::Response {
        self.api_client
            .post(format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn login(&self) {
        self.post_login(&json!({
            "username": self.test_user.username,
            "password": self.test_user.password,
        }))
        .await;
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(format!("{}/admin/logout", &self.address))
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

    pub async fn get_admin_dashboard(&self) -> Response {
        self.api_client
            .get(format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to post request")
    }

    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutput::NoAvaliableTask =
                try_execute_task(&self.pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }

    pub async fn clean_all_idempotency(&self) {
        try_clean_expired_idempotency(&self.pool, &Duration::from_secs(0))
            .await
            .unwrap();
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

    let app_config = get_test_config(email_server.uri());

    let pool = configure_database(&app_config.database).await;

    let app = Application::build(app_config.clone())
        .await
        .expect("Failed to build application");

    let app_port = app.port();
    let app_url = format!("http://127.0.0.1:{}", app_port);

    tokio::spawn(app.run_until_stop());

    let test_user = TestUser::generate();
    test_user.store(&pool).await;

    let api_client = reqwest::Client::builder()
        .cookie_store(true)
        // Do not follow redirects automatically
        .redirect(Policy::none())
        .build()
        .unwrap();

    let email_client = app_config.email_client.client();

    TestApp {
        address: app_url,
        port: app_port,
        pool,
        email_server,
        test_user,
        api_client,
        email_client,
    }
}

fn get_test_config(email_server_uri: String) -> Settings {
    let mut c = craft::configuration::get_config()
        .expect("Failed to load configuration");

    c.database.database_name = format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "_")
    );
    c.email_client.base_url = email_server_uri;
    c.email_client.retries_limit = 2;
    c.email_client.retry_wait_seconds = 1;

    c
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

pub fn assert_is_redirect_to(response: &reqwest::Response, url: &str) {
    let status = response.status();
    assert!(
        status.is_redirection(),
        "Expected redirect status, got {}",
        status
    );

    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .expect("Invalid Location header value");

    assert_eq!(
        location, url,
        "Expected redirect to `{}`, but got `{}`",
        url, location
    );
}
