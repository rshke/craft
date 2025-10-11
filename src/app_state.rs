use sqlx::{Pool, Postgres};

use crate::email_client::EmailClient;

pub struct AppState {
    pub pool: Pool<Postgres>,
    pub email_client: EmailClient,
    pub base_url: String,
}
