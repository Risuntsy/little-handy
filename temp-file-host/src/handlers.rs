use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::signal;
use urlencoding::encode;

use crate::{config::AppState, models::AppError, services::save_file};

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    filename: String,
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            let (original_filename, sha256_hash) = save_file(&state, field).await?;

            let download_url = format!(
                "{}/download/{}?filename={}",
                state.base_url.trim_end_matches('/'),
                sha256_hash,
                encode(&original_filename)
            );

            return Ok(download_url);
        }
    }

    Err(AppError::UserError(
        "No 'file' field found in multipart form data".to_string(),
    ))
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(short_hash): Path<String>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, AppError> {
    // 验证哈希格式 (16位十六进制)
    if !utils_share::http::validate_hash_format(&short_hash) {
        tracing::warn!("Attempt to download with invalid hash: {}", short_hash);
        return Err(AppError::NotFound("Invalid file identifier".to_string()));
    }

    let file_path = state.upload_path.join(&short_hash);
    if !file_path.exists() {
        tracing::warn!("File not found for download: {:?}", file_path);
        return Err(AppError::NotFound("File not found".to_string()));
    }

    let safe_filename = utils_share::http::sanitize_filename(&query.filename);
    let headers = HeaderMap::from_iter(vec![
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename*=UTF-8''{}",
                urlencoding::encode(&safe_filename)
            ))
            .unwrap_or_else(|_| {
                HeaderValue::from_str(&format!(
                    "attachment; filename=\"{}\"",
                    safe_filename.replace('\"', "\\\"")
                ))
                .unwrap()
            }),
        ),
    ]);

    let body = Body::from_stream(tokio_util::io::ReaderStream::new(
        tokio::fs::File::open(&file_path).await?,
    ));

    tracing::info!("Sending file {:?} as '{}'", file_path, safe_filename);
    Ok((headers, body).into_response())
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

pub async fn health_check() -> &'static str {
    "OK"
}
