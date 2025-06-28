use anyhow::Result;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use hex::ToHex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::OwnedSemaphorePermit;
use tracing::{error, info, instrument};
use urlencoding;
use uuid::Uuid;

use crate::{config::AppState, models::AppError, services::save_file_from_bytes};

#[derive(Debug, Deserialize)]
pub struct ProxyRequest {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

impl ProxyRequest {
    pub fn generate_cache_key(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.url.as_bytes());
        hasher.update(self.method.as_deref().unwrap_or("GET").as_bytes());
        for (key, value) in &self.headers {
            hasher.update(key.as_bytes());
            hasher.update(value.as_bytes());
        }
        if let Some(body) = &self.body {
            hasher.update(body.as_bytes());
        }
        hasher.finalize().encode_hex()
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Pending,
    Downloading,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Clone)]
pub struct JobStatus {
    pub job_id: Uuid,
    pub state: JobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProxyResponse {
    pub job_id: Uuid,
    pub status_url: String,
}

#[instrument(skip_all, fields(url = %request.url, job_id))]
pub async fn start_proxy_download(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProxyRequest>,
) -> Result<Json<ProxyResponse>, AppError> {
    let cache_key = request.generate_cache_key();

    if let Some(cached_status) = state.job_cache.get(&cache_key).await {
        if let JobState::Completed = cached_status.state {
            info!(cache_key, "Returning completed job from cache");
            return Ok(Json(ProxyResponse {
                job_id: cached_status.job_id,
                status_url: format!(
                    "{}/proxy/status/{}",
                    state.base_url.trim_end_matches('/'),
                    cached_status.job_id
                ),
            }));
        }
    }

    let permit = state
        .job_semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| {
            AppError::ServiceUnavailable("No available slots for new download jobs".to_string())
        })?;

    let job_id = Uuid::new_v4();
    tracing::Span::current().record("job_id", &tracing::field::display(job_id));

    let job_status = Arc::new(JobStatus {
        job_id,
        state: JobState::Pending,
        final_url: None,
        error_message: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });

    state.job_status_map.insert(job_id, job_status.clone());
    state.job_cache.insert(cache_key, job_status.clone()).await;

    tokio::spawn(run_download_job(state.clone(), request, job_id, permit));

    info!("Started new proxy download job");
    Ok(Json(ProxyResponse {
        job_id,
        status_url: format!(
            "{}/proxy/status/{}",
            state.base_url.trim_end_matches('/'),
            job_id
        ),
    }))
}

pub async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobStatus>, AppError> {
    match state.job_status_map.get(&job_id) {
        Some(status) => Ok(Json(status.value().as_ref().clone())),
        None => Err(AppError::NotFound(format!(
            "Job with ID {} not found",
            job_id
        ))),
    }
}

#[instrument(skip_all, fields(job_id, url = %request.url))]
async fn run_download_job(
    state: Arc<AppState>,
    request: ProxyRequest,
    job_id: Uuid,
    _permit: OwnedSemaphorePermit,
) {
    info!("Starting download job execution");

    let update_and_store_status = |state: &Arc<AppState>,
                                   job_id: Uuid,
                                   new_state: JobState,
                                   error: Option<String>,
                                   final_url: Option<String>| {
        let new_status = JobStatus {
            job_id,
            state: new_state,
            final_url,
            error_message: error,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let status_arc = Arc::new(new_status);
        state.job_status_map.insert(job_id, status_arc.clone());
        status_arc
    };

    // Update to Downloading
    update_and_store_status(&state, job_id, JobState::Downloading, None, None);

    let download_result = download_from_remote(
        &state.http_client,
        &request,
        state.proxy_config.remote_request_timeout_seconds,
    )
    .await;

    match download_result {
        Ok(bytes) => {
            info!("Successfully downloaded content, now processing and saving.");
            update_and_store_status(&state, job_id, JobState::Processing, None, None);

            let original_filename = request
                .url
                .split('/')
                .last()
                .unwrap_or("downloaded_file")
                .to_string();
            match save_file_from_bytes(&state, bytes, &original_filename).await {
                Ok(file_meta) => {
                    let final_url = format!(
                        "{}/download/{}?filename={}",
                        state.base_url.trim_end_matches('/'),
                        file_meta.short_hash,
                        urlencoding::encode(&file_meta.original_filename)
                    );
                    info!("File saved successfully. Final URL: {}", final_url);
                    let final_status = update_and_store_status(
                        &state,
                        job_id,
                        JobState::Completed,
                        None,
                        Some(final_url),
                    );
                    state
                        .job_cache
                        .insert(request.generate_cache_key(), final_status)
                        .await;
                }
                Err(e) => {
                    error!("Failed to save downloaded file: {:?}", e);
                    update_and_store_status(
                        &state,
                        job_id,
                        JobState::Failed,
                        Some(format!("Failed to save file: {}", e)),
                        None,
                    );
                }
            }
        }
        Err(e) => {
            error!("Failed to download from remote URL: {:?}", e);
            update_and_store_status(&state, job_id, JobState::Failed, Some(e.to_string()), None);
        }
    }
}

async fn download_from_remote(
    client: &Client,
    request: &ProxyRequest,
    timeout_secs: u64,
) -> Result<Bytes> {
    let method = match request
        .method
        .as_deref()
        .unwrap_or("GET")
        .to_uppercase()
        .as_str()
    {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        _ => anyhow::bail!("Unsupported HTTP method"),
    };

    let mut req_builder = client.request(method, &request.url);

    for (key, value) in &request.headers {
        req_builder = req_builder.header(key, value);
    }

    if let Some(body) = &request.body {
        req_builder = req_builder.body(body.clone());
    }

    let response = req_builder
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Remote server returned status: {}", response.status());
    }

    let bytes = response.bytes().await?;
    Ok(bytes)
}
