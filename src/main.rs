use std::fmt::{Debug, Display};
use tokio::task::JoinError;

use craft::configuration::get_config;
use craft::issue_delivery_worker::run_worker_until_stop;
use craft::startup::Application;
use craft::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber =
        get_subscriber("craft".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    dotenvy::dotenv().ok();

    let settings = get_config().expect("Failed to load configuration");

    let app = Application::build(settings)
        .await
        .expect("Failed to build application");
    let app_task = tokio::spawn(app.run_until_stop());

    // TODO:    We are not able to clone settings as it contains SecretBox<String>.
    //          Maybe there is a way to avoid this duplicated get_config
    let settings = get_config().expect("Failed to load configuration");
    let worker_task = tokio::spawn(run_worker_until_stop(settings));

    tokio::select! {
        o = app_task => report_exit("API", o),
        o = worker_task => report_exit("Background worker", o)
    };

    Ok(())
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl Debug + Display>, JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed", task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}
