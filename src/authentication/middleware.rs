use axum::{
    extract::{FromRequestParts, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct UserId(Uuid);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UserId {
    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

pub async fn reject_anonymous_users(request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let session =
        match crate::routers::session_state::TypeSession::from_request_parts(
            &mut parts,
            &(),
        )
        .await
        {
            Ok(session) => session,
            Err(_) => {
                return axum::response::Redirect::to("/login").into_response();
            }
        };

    let mut request = Request::from_parts(parts, body);
    match session.get_user_id() {
        Some(user_id) => {
            tracing::debug!("Authenticated user with ID: {}", user_id);
            request.extensions_mut().insert(UserId(user_id));
            next.run(request).await
        }
        None => {
            tracing::info!(
                "Anonymous user attempted to access a protected route."
            );
            axum::response::Redirect::to("/login").into_response()
        }
    }
}
