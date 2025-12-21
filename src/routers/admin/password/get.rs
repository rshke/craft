use axum::response;

pub async fn change_password_form() -> response::Html<&'static str> {
    axum::response::Html(include_str!("change_password.html"))
}
