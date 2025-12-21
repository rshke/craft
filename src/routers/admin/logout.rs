use axum::{
    Extension,
    response::{IntoResponse, Redirect, Response},
};

use crate::{authentication::UserId, routers::session_state::TypeSession};

pub async fn logout(
    session: TypeSession,
    Extension(user_id): Extension<UserId>,
) -> Response {
    let user_id = user_id.into_inner();

    session.logout();
    tracing::info!("User {user_id} logged out successfully.");
    Redirect::to("/login").into_response()
}
