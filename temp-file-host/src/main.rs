use axum::{
    Router,
    extract::DefaultBodyLimit,
    response::Html,
    routing::{get, post},
};

use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::fs;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use temp_file_host::{config::AppState, config::Config, handlers};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let config = Config::new()?;

    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| config.logging.level.clone()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_ansi(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
        )
        .init();

    let upload_path = PathBuf::from(&config.storage.upload_dir);
    let _ = fs::create_dir_all(&upload_path).await;
    info!("Upload directory ensured at: {:?}", upload_path);

    let app_state = Arc::new(AppState {
        upload_path: upload_path.to_owned(),
        base_url: config.server.base_url.to_owned(),
        max_file_size: config.max_file_size_bytes(),
    });

    let sched = JobScheduler::new().await?;

    let app_state_for_cleanup_job = app_state.clone();
    let cleanup_job = Job::new_async(&config.storage.cleanup_schedule, move |_uuid, _l| {
        let path = app_state_for_cleanup_job.upload_path.to_owned();
        let days = config.storage.cleanup_days;
        Box::pin(async move {
            info!("Running cleanup job for files older than {} days...", days);
            match temp_file_host::services::cleanup_old_files(&path, days).await {
                Ok(count) => info!("Cleanup finished. Deleted {} old files.", count),
                Err(e) => error!("Cleanup job failed: {}", e),
            }
        })
    })?;

    sched.add(cleanup_job).await?;
    sched.start().await?;

    info!(
        "Cleanup scheduler started. Schedule: '{}'",
        config.storage.cleanup_schedule
    );

    let router = Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../static/index.html")) }),
        )
        .route("/upload", post(handlers::upload_file))
        .route("/download/{sha256_hash}", get(handlers::download_file))
        .route("/health", get(handlers::health_check))
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(config.max_file_size_bytes()))
        .with_state(app_state);

    let addr: SocketAddr = config.server.listen_addr.parse()?;
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(handlers::shutdown_signal())
        .await?;

    Ok(())
}
