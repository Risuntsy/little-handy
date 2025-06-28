use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{Html, Response},
    routing::{get, post},
    Router,
};
use dashmap::DashMap;
use moka::future::Cache;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use temp_file_host::{
    config::{AppState, Config},
    handlers, proxy,
};
use tokio::sync::Semaphore;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

async fn auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        if let Some(token) = utils_share::validation::extract_bearer_token(auth_header) {
            if state.auth_config.allowed_tokens.contains(&token.to_string()) {
                return Ok(next.run(req).await);
            }
        }
    }

    warn!("Unauthorized access attempt");
    Err(StatusCode::UNAUTHORIZED)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let config = Config::new()?;

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
                .with_line_number(true),
        )
        .init();

    let upload_path = PathBuf::from(&config.storage.upload_dir);
    tokio::fs::create_dir_all(&upload_path).await?;
    info!("Upload directory ensured at: {:?}", upload_path);

    let proxy_config = Arc::new(config.proxy.clone());
    let auth_config = Arc::new(config.auth.clone());

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(
            proxy_config.remote_request_timeout_seconds,
        ))
        .build()?;

    let job_cache = Cache::builder()
        .time_to_live(Duration::from_secs(proxy_config.cache_ttl_seconds))
        .max_capacity(proxy_config.cache_max_capacity)
        .build();

    let app_state = Arc::new(AppState {
        upload_path: upload_path.clone(),
        base_url: config.server.base_url.clone(),
        max_file_size: config.max_file_size_bytes(),
        http_client,
        proxy_config: proxy_config.clone(),
        job_cache,
        job_status_map: Arc::new(DashMap::new()),
        job_semaphore: Arc::new(Semaphore::new(proxy_config.max_concurrent_jobs)),
        auth_config,
    });

    let sched = JobScheduler::new().await?;
    let app_state_for_cleanup = app_state.clone();
    let cleanup_days = config.storage.cleanup_days;
    let cleanup_schedule = config.storage.cleanup_schedule.clone();

    let cleanup_job = Job::new_async(&cleanup_schedule, move |_uuid, _l| {
        let path = app_state_for_cleanup.upload_path.clone();
        Box::pin(async move {
            info!(
                "Running cleanup job for files older than {} days...",
                cleanup_days
            );
            match temp_file_host::services::cleanup_old_files(&path, cleanup_days).await {
                Ok(count) => info!("Cleanup finished. Deleted {} old files.", count),
                Err(e) => error!("Cleanup job failed: {}", e),
            }
        })
    })?;

    sched.add(cleanup_job).await?;
    sched.start().await?;
    info!(
        "Cleanup scheduler started. Schedule: '{}'",
        cleanup_schedule
    );

    let proxy_router = Router::new()
        .route("/download", post(proxy::start_proxy_download))
        .route("/status/:job_id", get(proxy::get_job_status))
        .route_layer(middleware::from_fn_with_state(app_state.clone(), auth));

    let app_router = Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../static/index.html")) }),
        )
        .route("/upload", post(handlers::upload_file))
        .route("/download/:sha256_hash", get(handlers::download_file))
        .route("/health", get(handlers::health_check))
        .nest("/proxy", proxy_router)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(config.max_file_size_bytes()))
        .with_state(app_state);

    let addr: SocketAddr = config.server.listen_addr.parse()?;
    info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app_router)
        .with_graceful_shutdown(handlers::shutdown_signal())
        .await?;

    Ok(())
}
