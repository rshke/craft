use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub email: String,
}

pub(crate) async fn subscript(State(pool): State<PgPool>, Json(user): Json<User>) -> StatusCode {
    if !is_valid_name(&user.name) {
        return StatusCode::BAD_REQUEST;
    }

    println!("User subscribed: {user:?}");
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, name, email, subscribed_at)
        VALUES (gen_random_uuid(), $1, $2, NOW())
        "#,
        user.name,
        user.email
    )
    .execute(&pool)
    .await
    .expect("Failed to insert subscription");

    StatusCode::OK
}

fn is_valid_name(name: &str) -> bool {
    let is_empty = name.trim().is_empty();
    let is_too_long = name.len() > 256;
    let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
    let contains_forbidden_characters = name.chars().any(|g| forbidden_characters.contains(&g));

    !(is_empty || is_too_long || contains_forbidden_characters)
}
