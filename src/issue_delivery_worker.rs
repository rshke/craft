use std::time::Duration;

use crate::{configuration::Settings, email_client::EmailClient};
use sqlx::{PgPool, Postgres, Transaction};
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
            Ok(ExecutionOutput::EmptyEqueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await
            }
            Err(_) => {
                // Improvement:
                // https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/
                tokio::time::sleep(Duration::from_secs(2)).await
            }
            Ok(ExecutionOutput::TaskCompleted) => {}
        }
    }
}

pub enum ExecutionOutput {
    TaskCompleted,
    EmptyEqueue,
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
    if let Some((tx, issue_id, subscriber_email)) = dequeue_task(pool).await? {
        Span::current()
            .record("issue_id", display(&issue_id))
            .record("subscriber_email", display(&subscriber_email));

        match subscriber_email.parse::<SubscriberEmail>() {
            Ok(subscriber_email) => {
                let (title, text_content, html_content) =
                    get_issue(pool, issue_id).await?;
                let re = email_client
                    .send_email(
                        &subscriber_email,
                        &title,
                        &text_content,
                        &html_content,
                    )
                    .await;

                // TODO: try n times before skip
                if let Err(e) = re {
                    tracing::error!(
                        error.cause_chain = ?e,
                        error.message = %e,
                        "Failed to deliver issue to a confirmed subscriber. \
                        Skipping.",
                    );
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

        delete_task(tx, issue_id, subscriber_email).await?;
    } else {
        return Ok(ExecutionOutput::EmptyEqueue);
    }

    Ok(ExecutionOutput::TaskCompleted)
}

#[instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<
    Option<(Transaction<'static, Postgres>, uuid::Uuid, String)>,
    anyhow::Error,
> {
    let mut tx = pool.begin().await?;

    // KEY: FOR UPDATE SKIP LOCKED
    let record = sqlx::query!(
        r#"
        SELECT
            newsletter_issue_id,
            subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(r) = record {
        return Ok(Some((tx, r.newsletter_issue_id, r.subscriber_email)));
    }

    Ok(None)
}

#[instrument(skip_all)]
async fn delete_task(
    mut tx: Transaction<'static, Postgres>,
    issue_id: uuid::Uuid,
    email: String,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
            newsletter_issue_id = $1 AND
            subscriber_email = $2
        "#,
        issue_id,
        email,
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
    issue_id: uuid::Uuid,
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
