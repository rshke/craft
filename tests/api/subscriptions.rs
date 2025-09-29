use std::collections::HashMap;

use crate::helper::spawn_app;

#[tokio::test]
async fn subscript_works() {
    let app = spawn_app().await;

    let mut map = HashMap::new();
    map.insert("name", "rust");
    map.insert("email", "noisy_drop@example.com");

    let response = app.post_subscriptions(&map).await;

    println!("Response: {response:?}");
    assert!(response.status().is_success(), "Subscription failed");

    let saved = sqlx::query!(
        r#"
         SELECT email, name
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
}

#[tokio::test]
async fn subscrpit_return_422_err_for_incorrect_fields() {
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
