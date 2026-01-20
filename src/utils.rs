use axum::{http::StatusCode, response::IntoResponse};

use crate::routers::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    E500(#[from] anyhow::Error),
    #[error("syntax error in request")]
    E400(#[source] anyhow::Error),
    #[error("authorization failed")]
    E401(#[source] anyhow::Error),
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::E500(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::E400(_) => StatusCode::BAD_REQUEST,
            Self::E401(_) => StatusCode::UNAUTHORIZED,
        }
    }
}

impl std::fmt::Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        #[derive(serde::Serialize)]
        struct ErrorResponse {
            message: String,
            details: String,
        }

        let message = format!("{self}");
        let details = format!("{:?}", self);
        let body = axum::Json(ErrorResponse { message, details });

        let status_code = self.status_code();
        let mut response = (status_code, body).into_response();
        response
            .extensions_mut()
            .insert(std::sync::Arc::new(anyhow::anyhow!(self)));

        response
    }
}
