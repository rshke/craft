use axum::response;

pub async fn login_form() -> response::Html<&'static str> {
    axum::response::Html(include_str!("login.html"))
}
