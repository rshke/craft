use std::collections::HashMap;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::helper::spawn_app;

fn valid_subscriber() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("name", "rust");
    map.insert("email", "noisy_drop@example.com");

    map
}

#[tokio::test]
async fn subscript_return_200_for_valid_data() {
    let app = spawn_app().await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let map = valid_subscriber();

    let response = app.post_subscriptions(&map).await;

    println!("Response: {response:?}");
    assert!(response.status().is_success(), "Subscription failed");
}

#[tokio::test]
async fn subscribe_persist_the_new_subscription() {
    let app = spawn_app().await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let map = valid_subscriber();

    app.post_subscriptions(&map).await;

    let saved = sqlx::query!(
        r#"
         SELECT email, name, status
         FROM subscriptions;
    "#
    )
    .fetch_one(&app.pool)
    .await
    .expect("Failed to fetch saved subscription");
    assert!(saved.email == map["email"]);
    assert!(
        saved.name == "rust",
        "Expected name to be 'rust', got '{}'",
        saved.name
    );
    assert!(
        saved.status == "pending_confirmation",
        "Expected status to be 'pending_confirmation', get '{}'",
        saved.status
    )
}

#[tokio::test]
async fn subscribe_return_422_err_for_incorrect_fields() {
    let app = spawn_app().await;
    // table-driven test
    let invalid_users_field =
        vec![("name_", "email"), ("name", "email_"), ("name_", "email_")];

    for (name_field, email_field) in invalid_users_field {
        let mut map = HashMap::new();
        map.insert(name_field, "rust");
        map.insert(email_field, "json");

        let response = app.post_subscriptions(&map).await;

        assert_eq!(
            response.status(),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "Expected 422 error for invalid user data"
        );
    }
}

#[tokio::test]
async fn subscript_return_422_err_for_incorrect_values() {
    let app = spawn_app().await;
    let invalid_users_field = vec![
        ("", "email@gmail.com"),
        ("  ", "email@gmail.com"),
        ("Noisy Drop", ""),
        ("Noisy Drop", "@gmail.com"),
        ("Noisy Drop", "noisy_drop.gmail.com"),
    ];

    for (name, email) in invalid_users_field {
        let mut map = HashMap::new();
        map.insert("name", name);
        map.insert("email", email);

        let response = app.post_subscriptions(&map).await;

        assert_eq!(
            response.status(),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "Expected 422 error for invalid user data: {map:?}"
        );
    }
}

#[tokio::test]
async fn send_confirm_email_for_valid_subscription() {
    let app = spawn_app().await;
    let subscriber = valid_subscriber();

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(&subscriber).await;
}

#[tokio::test]
async fn confirm_email_contains_link() {
    let app = spawn_app().await;
    let subscriber = valid_subscriber();

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(&subscriber).await;

    let request = &app.email_server.received_requests().await.unwrap()[0];

    let links = app.extract_links(request);

    assert_eq!(links.html, links.pain_text);
}
