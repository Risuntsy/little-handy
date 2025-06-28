use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};

use repo_host::{
    config::{get_config, get_service_config},
    git::clone_or_pull_service_repo,
    server,
    util::generate_my_hash,
};
use log::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Starts the file server
    Server,
    /// Generates a signed URL for a given path
    UrlOf {
        /// The path to generate a URL for
        path: String,
    },
}

fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cli = Cli::parse();

    match cli.command {
        Commands::Server => {
            server::run_server().expect("Error starting server: {}");
        }
        Commands::UrlOf { path } => {
            let config = get_config();

            let service_name = path.split("/").next().expect("Failed to get service name");

            let service_config = get_service_config(service_name);

            info!("Generating signed URL for {:?}", &path);

            clone_or_pull_service_repo(service_name).expect("Failed to clone or pull repo");

            let url_of = |path: &str| {
                if let Some(secret_key) = &service_config.secret_key {
                    format!(
                        "https://{}/{}?sign={}",
                        config.server.domain,
                        PathBuf::from_str(path)
                            .expect("Failed to parse path")
                            .to_str()
                            .expect("Failed to convert path to string"),
                        generate_my_hash(path, secret_key)
                    )
                } else {
                    format!(
                        "https://{}/{}",
                        config.server.domain,
                        PathBuf::from_str(path)
                            .expect("Failed to parse path")
                            .to_str()
                            .expect("Failed to convert path to string"),
                    )
                }
            };

            println!("access url:\n{}", url_of(&path))
        }
    }
}
