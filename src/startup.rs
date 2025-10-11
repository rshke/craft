use axum::Router;
use axum::routing::IntoMakeService;
use axum::serve::Serve;
use sqlx::PgPool;

use crate::configuration::Settings;
use crate::email_client::EmailClient;
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

        let email_client = EmailClient::new(
            settings.email_client.base_url,
            settings.email_client.sender,
            settings.email_client.authorization_token,
            settings.email_client.timeout_milliseconds,
        );

        let app = routers::get_router(
            pool,
            email_client,
            settings.app_settings.base_url,
        );
        let server = axum::serve(listener, app.into_make_service());

        Ok(Self {
            port: server.local_addr()?.port(),
            server,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stop(self) {
        tracing::info!("Listening on {}", self.server.local_addr().unwrap());
        let _ = self.server.await;
    }
}
