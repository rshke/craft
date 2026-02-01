use std::time::Duration;

use crate::configuration::Settings;
use sqlx::PgPool;
use tracing::instrument;

pub async fn run_expire_clean_worker_until_stop(
    settings: Settings,
) -> Result<(), anyhow::Error> {
    let db_url = settings.database.get_connection();
    let pool = PgPool::connect_lazy(&db_url)
        .expect("Failed to connect to the database");

    work_loop(pool, settings.app_settings.idempotency_ttl).await
}

#[instrument(skip_all)]
async fn work_loop(pool: PgPool, ttl: Duration) -> Result<(), anyhow::Error> {
    loop {
        match try_clean_expired_idempotency(&pool, &ttl).await {
            Err(e) => {
                tracing::error!(
                    error.cause_chain=?e,
                    error.message=%e,
                    "failed to clean expired idempotency records"
                )
            }
            Ok(deleted_rows) => tracing::info!(
                "{} expired idempotency records are cleaned",
                deleted_rows,
            ),
        }

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

#[instrument(skip_all)]
pub async fn try_clean_expired_idempotency(
    pool: &PgPool,
    ttl: &Duration,
) -> Result<u64, anyhow::Error> {
    let expired_at = time::OffsetDateTime::now_local()? - *ttl;
    let deleted_rows = sqlx::query!(
        r#"
        DELETE
        FROM idempotency
        WHERE created_at < $1
        "#,
        expired_at
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok(deleted_rows)
}
