use crate::{configuration::Settings, email_client::EmailClient};
use sqlx::{PgPool, Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use tracing::{Span, field::display, instrument};

use crate::domain::subscriber_email::SubscriberEmail;

pub async fn run_worker_until_stop(
    settings: Settings,
) -> Result<(), anyhow::Error> {
    let db_url = settings.database.get_connection();
    let pool = PgPool::connect_lazy(&db_url)
        .expect("Failed to connect to the database");

    let email_client = settings.email_client.client();

    work_loop(pool, email_client).await
}

async fn work_loop(
    pool: PgPool,
    email_client: EmailClient,
) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutput::NoAvaliableTask) => {
                tokio::time::sleep(Duration::seconds(10).try_into().unwrap())
                    .await
            }
            Err(_) => {
                // Improvement:
                // https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/
                tokio::time::sleep(Duration::seconds(2).try_into().unwrap())
                    .await
            }
            Ok(ExecutionOutput::TaskCompleted) => {}
            Ok(ExecutionOutput::RetryLater) => {}
        }
    }
}

pub enum ExecutionOutput {
    TaskCompleted,
    NoAvaliableTask,
    RetryLater,
}

#[instrument(
    skip_all,
    fields(
        issue_id = tracing::field::Empty,
        subscriber_email = tracing::field::Empty,
    ),
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutput, anyhow::Error> {
    if let Some((tx, task)) = dequeue_task(pool).await? {
        Span::current()
            .record("issue_id", display(&task.newsletter_issue_id))
            .record("subscriber_email", display(&task.subscriber_email));

        match task.subscriber_email.parse::<SubscriberEmail>() {
            Ok(subscriber_email) => {
                let (title, text_content, html_content) =
                    get_issue(pool, &task.newsletter_issue_id).await?;
                let re = email_client
                    .send_email(
                        &subscriber_email,
                        &title,
                        &text_content,
                        &html_content,
                    )
                    .await;

                if let Err(e) = re {
                    if u16::try_from(task.n_retries).unwrap() + 1
                        >= email_client.retries_limit
                    {
                        tracing::warn!(
                            error.cause_chain = ?e,
                            error.message = %e,
                            "Failed to deliver issue to a confirmed subscriber. \
                            Exceed max retry times. Cancel delivery",
                        );
                    } else {
                        tracing::warn!(
                            error.cause_chain = ?e,
                            error.message = %e,
                            "Failed to deliver issue to a confirmed subscriber. \
                            Retrying later.",
                        );
                        schedule_next_retry(
                            tx,
                            &task,
                            email_client.retry_wait_seconds,
                        )
                        .await?;

                        return Ok(ExecutionOutput::RetryLater);
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    error.cause_chain=?e,
                    error.message=%e,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                )
            }
        }

        delete_task(tx, task).await?;
    } else {
        return Ok(ExecutionOutput::NoAvaliableTask);
    }

    Ok(ExecutionOutput::TaskCompleted)
}

struct DeliveryTask {
    newsletter_issue_id: uuid::Uuid,
    subscriber_email: String,
    n_retries: i16,
    execute_after: OffsetDateTime,
}

#[instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(Transaction<'static, Postgres>, DeliveryTask)>, anyhow::Error>
{
    let mut tx = pool.begin().await?;

    // KEY: FOR UPDATE SKIP LOCKED
    let record = sqlx::query_as!(
        DeliveryTask,
        r#"
        SELECT
            newsletter_issue_id,
            subscriber_email,
            n_retries,
            execute_after
        FROM issue_delivery_queue
        WHERE execute_after < NOW()
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(r) = record {
        return Ok(Some((tx, r)));
    }

    Ok(None)
}

#[instrument(skip_all)]
async fn schedule_next_retry(
    mut tx: Transaction<'static, Postgres>,
    task: &DeliveryTask,
    retry_wait_seconds: u16,
) -> Result<(), anyhow::Error> {
    let next_delivery =
        task.execute_after + Duration::seconds(retry_wait_seconds.into());
    sqlx::query!(
        r#"
        UPDATE issue_delivery_queue
        SET
            n_retries = $1,
            execute_after = $2
        WHERE
            newsletter_issue_id = $3 AND
            subscriber_email = $4
        "#,
        task.n_retries + 1,
        next_delivery,
        task.newsletter_issue_id,
        task.subscriber_email,
    )
    .execute(&mut *tx)
    .await?;

    let _ = tx.commit().await;

    Ok(())
}

#[instrument(skip_all)]
async fn delete_task(
    mut tx: Transaction<'static, Postgres>,
    task: DeliveryTask,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
            newsletter_issue_id = $1 AND
            subscriber_email = $2
        "#,
        task.newsletter_issue_id,
        task.subscriber_email,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

#[instrument(skip_all)]
async fn get_issue(
    // some read-only queries should use the pool instead of the transaction
    // to avoid deadlocks and improve performance
    pool: &PgPool,
    issue_id: &uuid::Uuid,
) -> Result<(String, String, String), anyhow::Error> {
    let record = sqlx::query!(
        r#"
        SELECT
            title,
            text_content,
            html_content
        FROM newsletter_issues
        WHERE newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;

    Ok((record.title, record.text_content, record.html_content))
}
