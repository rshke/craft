use axum::Router;
use axum::routing::IntoMakeService;
use axum::serve::Serve;
use axum_session::{SessionConfig, SessionStore};
use axum_session_redispool::SessionRedisPool;
use redis_pool::RedisPool;
use secrecy::ExposeSecret;
use sqlx::PgPool;

use crate::configuration::Settings;
use crate::routers;

type Server = Serve<tokio::net::TcpListener, IntoMakeService<Router>, Router>;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(settings: Settings) -> Result<Self, std::io::Error> {
        let addr = std::net::SocketAddr::from((
            settings.app_settings.host,
            settings.app_settings.port,
        ));
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        let db_url = settings.database.get_connection();
        let pool = PgPool::connect_lazy(&db_url)
            .expect("Failed to connect to the database");

        let email_client = settings.email_client.client();

        let session_store = Self::get_redis_store(
            settings.app_settings.redis_url.expose_secret(),
        )
        .await;

        let app = routers::get_router(
            pool,
            email_client,
            settings.app_settings.base_url,
            session_store,
        );
        let server = axum::serve(listener, app.into_make_service());

        Ok(Self {
            port: server.local_addr()?.port(),
            server,
        })
    }

    async fn get_redis_store(
        redis_url: &str,
    ) -> SessionStore<SessionRedisPool> {
        let client = redis::Client::open(redis_url)
            .expect("Error while trying to open the redis connection");
        let redis_pool = RedisPool::from(client);
        let session_config = SessionConfig::default();

        SessionStore::<SessionRedisPool>::new(
            Some(redis_pool.clone().into()),
            session_config,
        )
        .await
        .expect("Failed to create redis session store.")
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stop(self) -> Result<(), anyhow::Error> {
        tracing::info!("Listening on {}", self.server.local_addr()?);
        let _ = self.server.await;
        Ok(())
    }
}
