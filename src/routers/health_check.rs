pub(crate) async fn health_check() -> &'static str {
    log::info!("Health check endpoint hit");
    "OK"
}
