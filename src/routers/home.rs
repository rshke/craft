use axum::response;

pub async fn home() -> response::Html<&'static str> {
    response::Html(include_str!("home/home.html"))
}
