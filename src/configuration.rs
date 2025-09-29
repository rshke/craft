use std::{fmt, str::FromStr};

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use secrecy::{ExposeSecret, SecretBox};

use crate::domain::subscriber_email::SubscriberEmail;

#[derive(Deserialize)]
pub struct Settings {
    pub app_settings: AppSettings,
    pub database: DBSettings,
    pub email_client: EmailClientSettings,
}

#[derive(Deserialize)]
pub struct AppSettings {
    pub host: [u8; 4], // IPv4 address
    pub port: u16,
}

#[derive(Deserialize)]
pub struct DBSettings {
    pub username: String,
    pub password: SecretBox<String>,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

#[derive(Deserialize)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender: SubscriberEmail,
    pub authorization_token: SecretBox<String>,
    #[serde(default = "default_timeout_milliseconds")]
    pub timeout_milliseconds: u64,
}

fn default_timeout_milliseconds() -> u64 {
    10_000
}

enum RunningEnv {
    Local,
    Production,
}

impl RunningEnv {
    pub fn as_str(&self) -> &str {
        match self {
            RunningEnv::Local => "local",
            RunningEnv::Production => "production",
        }
    }
}

impl fmt::Display for RunningEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RunningEnv {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(RunningEnv::Local),
            "production" => Ok(RunningEnv::Production),
            _ => Err("Invalid environment specified"),
        }
    }
}

impl DBSettings {
    pub fn get_connection(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        )
    }

    pub fn get_connection_without_database(&self) -> String {
        // postgres requires a db name when clients try to connect to it.
        // if no db specified, it will try to use the db that has same name as user name
        // here we use the builtin db postgres
        format!(
            "postgres://{}:{}@{}:{}/postgres",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port
        )
    }
}

pub fn get_config() -> Result<Settings, ConfigError> {
    let current_dir =
        std::env::current_dir().expect("Failed to get current directory");
    let config_path = current_dir.join("configurations");

    let running_env =
        std::env::var("RUNNING_ENV").unwrap_or_else(|_| "local".to_string());
    let running_env: RunningEnv =
        running_env.as_str().parse().unwrap_or_else(|err| {
            panic!("Failed to parse RUNNING_ENV: {err}");
        });

    let app_config_file = format!("{running_env}.yaml");
    let config = Config::builder()
        .add_source(File::with_name(
            config_path
                .join("base.yaml")
                .to_str()
                .expect("Invalid path"),
        ))
        .add_source(File::from(config_path.join(app_config_file)))
        .add_source(Environment::with_prefix("CRAFT").separator("__"))
        .build()?;

    // print actual configuration for debugging
    // println!("Configuration loaded: {config:#?}");
    config.try_deserialize::<Settings>()
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial] // Ensure tests run in order to avoid environment conflicts
    fn test_get_local_config() {
        unsafe {
            std::env::set_var("RUNNING_ENV", "local");
        }

        let settings = get_config().unwrap();
        assert_eq!(
            settings.app_settings.host,
            [127, 0, 0, 1],
            "Failed to load local configuration"
        );
        assert_eq!(
            settings.email_client.base_url, "localhost",
            "Failed to load local email client's base url"
        );
        assert_eq!(
            settings.email_client.timeout_milliseconds, 10_000,
            "Failed to load local configuration"
        );
    }

    #[test]
    #[serial]
    fn test_get_production_config() {
        unsafe {
            std::env::set_var("RUNNING_ENV", "production");
        }

        let settings = get_config().unwrap();
        assert_eq!(
            settings.app_settings.host,
            [0, 0, 0, 0],
            "Failed to load production configuration"
        );
    }

    #[test]
    #[serial]
    fn test_get_env_config() {
        unsafe {
            std::env::set_var("RUNNING_ENV", "production");
            std::env::set_var("CRAFT__DATABASE__PASSWORD", "abc123");
            std::env::set_var("CRAFT__DATABASE__USERNAME", "Alice");
        }

        let settings = get_config().unwrap();
        assert_eq!(
            settings.database.password.expose_secret(),
            "abc123",
            "Failed to load env configuration"
        );
        assert_eq!(
            settings.database.username, "Alice",
            "Failed to load env configuration"
        );
    }
}
