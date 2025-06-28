use std::{
    collections::HashMap,
    fs::{self},
    path::PathBuf,
    sync::LazyLock,
};

use serde::Deserialize;

static APP_CONFIG: LazyLock<Config> = LazyLock::new(init_config);
static SERVICE_CONFIG: LazyLock<HashMap<&'static str, AppServiceConfig>> =
    LazyLock::new(init_service_config);

fn init_config() -> Config {
    let config_content =
        fs::read_to_string("data/config.toml").expect("Failed to read config.toml");

    let config: Config = toml::from_str(&config_content).expect("Failed to parse config.toml");

    if !config.server.repo_store.exists() {
        fs::create_dir_all(&config.server.repo_store).expect("Failed to create repo store");
    }

    config
}

fn init_service_config() -> HashMap<&'static str, AppServiceConfig> {
    APP_CONFIG
        .services
        .iter()
        .map(|service| {
            (service.name.as_ref(), {
                let repo_path = APP_CONFIG.server.repo_store.join(&service.name);
                let private_key_path = service
                    .private_key
                    .as_ref()
                    .map(|p| APP_CONFIG.server.key_store.join(&service.name).join(p));

                AppServiceConfig {
                    name: service.name.to_owned(),
                    repo_url: service.repo_url.to_owned(),
                    repo_branch: service.repo_branch.to_owned(),
                    repo_path,
                    secret_key: private_key_path.as_ref().map(|p| {
                        format!(
                            "{}114514{}",
                            fs::read_to_string(p).expect("Failed to read private key"),
                            &service.github_webhook_secret
                        )
                    }),
                    github_webhook_secret: service.github_webhook_secret.to_owned(),
                    private_key_path,
                }
            })
        })
        .collect()
}

pub fn get_config() -> &'static Config {
    &APP_CONFIG
}

pub fn get_service_config(service_name: &str) -> &'static AppServiceConfig {
    &SERVICE_CONFIG
        .get(service_name)
        .expect(&format!("Service {} not found", service_name))
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub services: Vec<ServiceConfig>,
}

// repo private key = ./key/<service_name>/private_key_path

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub domain: String,
    pub repo_store: PathBuf,
    pub key_store: PathBuf,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ServiceConfig {
    pub name: String,
    pub repo_url: String,
    pub repo_branch: String,
    pub private_key: Option<PathBuf>,
    pub github_webhook_secret: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AppServiceConfig {
    pub name: String,
    pub repo_url: String,
    pub repo_branch: String,
    pub repo_path: PathBuf,
    pub private_key_path: Option<PathBuf>,
    pub github_webhook_secret: String,
    pub secret_key: Option<String>,
}
