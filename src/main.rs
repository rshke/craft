use tokio;


#[tokio::main]
async fn main() {
    let (app, listener) = craft::get_server().await;

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
