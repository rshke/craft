use axum::extract::FromRequestParts;
use axum::http::{StatusCode, request::Parts};
use axum_session::Session;
use axum_session_redispool::SessionRedisPool;
use uuid::Uuid;

pub struct TypeSession(Session<SessionRedisPool>);

impl TypeSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user_id(&self, user_id: Uuid) {
        self.0.set(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Option<Uuid> {
        self.0.get::<Uuid>(Self::USER_ID_KEY)
    }

    pub fn logout(&self) {
        self.0.destroy();
    }
}

impl<S> FromRequestParts<S> for TypeSession
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match Session::from_request_parts(parts, state).await {
            Ok(session) => Ok(TypeSession(session)),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to extract Session: {:?}", e),
            )),
        }
    }
}
