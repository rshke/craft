use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{any, method, path},
};

use crate::helper::{
    ConfirmationLinks, TestApp, assert_is_redirect_to, spawn_app,
    valid_subscriber,
};

#[tokio::test]
async fn requests_without_login_are_rejected() {
    let app = spawn_app().await;

    create_unconfirmed_subscriber(&app).await;

    let body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });
    let response = app.post_newsletters(&body).await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn donot_send_to_unconfirmed_subscribers() {
    let app = spawn_app().await;

    create_unconfirmed_subscriber(&app).await;
    app.post_login(&json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    }))
    .await;

    let body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        },
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(&body).await;

    assert_eq!(response.status(), StatusCode::OK)
}

#[tokio::test]
async fn send_to_confirmed_subscribers() {
    let app = spawn_app().await;

    create_confirmed_subscriber(&app).await;
    app.post_login(&json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    }))
    .await;

    let body = json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        },
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(&body).await;

    assert_eq!(response.status(), StatusCode::OK)
}

#[tokio::test]
async fn return_400_for_invalid_body() {
    let app = spawn_app().await;

    app.post_login(&json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    }))
    .await;

    let request_bodies = vec![
        (
            json!({
                "title": "Newsletter title"
            }),
            "missing conteint",
        ),
        (
            json!({
                "conteint": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>",
                }
            }),
            "missing title",
        ),
    ];

    for (body, flaw) in request_bodies {
        let response = app.post_newsletters(&body).await;

        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "The API did not fail with 400 Bad Request when the payload was {}.",
            flaw
        )
    }
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = valid_subscriber();

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(&body).await;

    let request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.extract_links(request)
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let app = spawn_app().await;
    app.post_login(&json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    }))
    .await;
    create_confirmed_subscriber(&app).await;

    let body = json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        },
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;
    let response = app.post_newsletters(&body).await;
    assert_eq!(response.status(), StatusCode::OK);

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;
    let response = app.post_newsletters(&body).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn concurrent_push_newsletter_is_handled_gracefully() {
    let app = spawn_app().await;
    app.post_login(&json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    }))
    .await;

    create_confirmed_subscriber(&app).await;

    let body = json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        },
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_delay(Duration::from_secs(2)),
        )
        .expect(1)
        .mount(&app.email_server)
        .await;

    let req1 = app.post_newsletters(&body);
    let re12 = app.post_newsletters(&body);

    let (response1, response2) = tokio::join!(req1, re12);

    assert_eq!(response1.status(), response2.status());
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirm_link = create_unconfirmed_subscriber(app).await.html;

    reqwest::get(confirm_link).await.unwrap();
}
