use anyhow::Context;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;

use crate::authentication::UserId;
use crate::idempotency::key::IdempotencyKey;

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>),
    ReturnSavedResponse(Response),
}

#[instrument(name = "Try to process", skip(pool, idempotency_key, user_id))]
pub async fn try_process(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: UserId,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    // Note: eable repeatable read will cause insert operation return Error
    // sqlx::query!("SET TRANSACTION ISOLATION LEVEL repeatable read")
    //     .execute(&mut *transaction)
    //     .await?;

    let n_insert_rows = sqlx::query!(
        r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1, $2, NOW())
        ON CONFLICT DO NOTHING
        "#,
        user_id.into_inner(),
        idempotency_key.as_ref()
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    if n_insert_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "We expected a saved response, we didn't find it"
                )
            })?;
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}

#[instrument(name = "Get saved response", skip(pool, idempotency_key, user_id))]
pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: UserId,
) -> Result<Option<Response>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
        SELECT
            response_status_code as "response_status_code!",
            response_headers as "response_headers!: Vec<HeaderPairRecord>",
            response_body as "response_body!"
        FROM idempotency
        WHERE idempotency_key = $1 AND user_id = $2
        "#,
        idempotency_key.as_ref(),
        user_id.into_inner(),
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch saved response")?;

    if let Some(r) = saved_response {
        let status_code =
            StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = Response::builder()
            .status(status_code)
            .body(Body::from(r.response_body))?;
        for HeaderPairRecord { name, value } in r.response_headers {
            let name = HeaderName::try_from(name)?;
            let value = HeaderValue::try_from(value)?;
            response.headers_mut().append(name, value);
        }

        Ok(Some(response))
    } else {
        Ok(None)
    }
}

#[instrument(
    name = "Save response",
    skip(tx, idempotency_key, user_id, response)
)]
pub async fn save_response(
    mut tx: Transaction<'static, Postgres>,
    idempotency_key: IdempotencyKey,
    user_id: UserId,
    response: Response,
) -> Result<Response, anyhow::Error> {
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, usize::MAX).await?;

    let status_code = parts.status.as_u16() as i16;

    let headers = {
        let headers = parts.headers.clone();
        let mut h = Vec::with_capacity(headers.len());
        let mut last_name = None;
        for (name, value) in headers {
            let name_str = match name {
                Some(name) => {
                    last_name = Some(name.as_str().to_string());
                    name.as_str().to_string()
                }
                None => last_name.clone().unwrap_or_default(),
            };

            h.push(HeaderPairRecord {
                name: name_str,
                value: value.as_bytes().to_vec(),
            });
        }
        h
    };

    sqlx::query_unchecked!(
        r#"
        UPDATE idempotency
        SET
            response_status_code = $1,
            response_headers = $2,
            response_body = $3
        WHERE
            user_id = $4 AND
            idempotency_key = $5
        "#,
        status_code,
        headers,
        body.to_vec(),
        user_id.into_inner(),
        idempotency_key.as_ref()
    )
    .execute(&mut *tx)
    .await
    .context("Failed to save response")?;

    tx.commit().await?;

    let mut response = Response::builder()
        .status(StatusCode::from_u16(status_code.try_into()?)?)
        .body(Body::from(body))?;
    response.headers_mut().extend(parts.headers);
    Ok(response)
}
