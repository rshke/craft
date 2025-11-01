use craft::domain::subscriber::SubscriberStatus;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::helper::{spawn_app, valid_subscriber};

#[tokio::test]
async fn reject_with_400_for_confirmation_without_token() {
    let app = spawn_app().await;

    let response =
        reqwest::get(format!("{}/subscriptions/confirm", app.address))
            .await
            .unwrap();

    assert_eq!(
        response.status().as_u16(),
        400,
        "Expect response return '400', but got {}",
        response.status().as_u16()
    )
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
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

    let response = reqwest::get(links.html).await.unwrap();

    assert_eq!(
        response.status().as_u16(),
        200,
        "Expect response return '200', but got {}",
        response.status().as_u16()
    );
}

#[tokio::test]
async fn clicking_the_link_confirms_the_subscription() {
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
    reqwest::get(links.html).await.unwrap();

    let saved = sqlx::query!(
        r#"
         SELECT email, name, status
         FROM subscriptions;
    "#
    )
    .fetch_one(&app.pool)
    .await
    .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, subscriber["email"]);
    assert_eq!(saved.name, subscriber["name"]);
    assert_eq!(saved.status, SubscriberStatus::Confirmed.to_string())
}
