use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::collections::HashMap;

use craft::configuration::DBConfig;
use craft::run;

#[tokio::test]
async fn subscript_works() {
    let (app_url, pool) = spawn_server().await;

    let mut map = HashMap::new();
    map.insert("name", "rust");
    map.insert("email", "json");

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{app_url}/subscriptions"))
        .json(&map)
        .send()
        .await
        .expect("Failed to send request");

    println!("Response: {response:?}");
    assert!(response.status().is_success(), "Subscription failed");

    let saved = sqlx::query!(
        r#"
         SELECT email, name
         FROM subscriptions;
    "#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch saved subscription");
    assert!(saved.email == "json");
    assert!(
        saved.name == "rust",
        "Expected name to be 'rust', got '{}'",
        saved.name
    );
}

#[tokio::test]
async fn subscrpit_return_422_err() {
    let (app_url, _) = spawn_server().await;
    // table-driven test
    let invalid_users_field = vec![("name_", "email"), ("name", "email_"), ("name_", "email_")];

    for (name_field, email_field) in invalid_users_field {
        let mut map = HashMap::new();
        map.insert(name_field, "rust");
        map.insert(email_field, "json");

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{app_url}/subscriptions"))
            .json(&map)
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(
            response.status(),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "Expected 422 Bad Request for invalid user data"
        );
    }
}

#[tokio::test]
async fn subscript_return_400_err() {
    let (app_url, _) = spawn_server().await;
    let invalid_users_field = vec![("", "a email"), ("  ", "a email")];

    for (name, email) in invalid_users_field {
        let mut map = HashMap::new();
        map.insert("name", name);
        map.insert("email", email);

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{app_url}/subscriptions"))
            .json(&map)
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(
            response.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "Expected 400 Bad Request for invalid user data"
        );
    }
}

async fn spawn_server() -> (String, PgPool) {
    let mut app_config = craft::configuration::get_config().expect("Failed to load configuration");
    app_config.database.database_name = format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "_")
    );
    let pool = configure_database(&app_config.database).await;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let app_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(run(listener, pool.clone()));

    (app_url, pool)
}

async fn configure_database(configuration: &DBConfig) -> PgPool {
    let mut db_connection =
        PgConnection::connect(configuration.get_connection_without_database().as_str())
            .await
            .expect("Failed to connect to postgres server");
    db_connection
        .execute(format!("CREATE DATABASE {};", configuration.database_name).as_str())
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
