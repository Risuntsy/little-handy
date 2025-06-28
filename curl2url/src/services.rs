use anyhow::Result;
use axum::{
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use std::{collections::HashMap, process::Command, sync::Arc};
use tracing::{error, info, warn};

use crate::{
    errors::AppError,
    models::{AppState, CurlResponse},
    utils::parse_curl_response,
};

pub async fn execute_curl_command(
    state: &Arc<AppState>,
    target_url: &str,
    headers: &HeaderMap,
    method: &str,
) -> Result<Response, AppError> {
    // 构建curl命令
    let mut curl_command = vec!["curl".to_string()];
    
    // 添加方法
    if method != "GET" {
        curl_command.push("-X".to_string());
        curl_command.push(method.to_string());
    }

    // 添加头部
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            curl_command.push("-H".to_string());
            curl_command.push(format!("{}: {}", name.as_str(), value_str));
        }
    }

    // 添加其他curl选项
    if state.config.curl.include_headers {
        curl_command.push("-i".to_string()); // 包含响应头
    }
    curl_command.push("-s".to_string()); // 静默模式
    if state.config.curl.follow_redirects {
        curl_command.push("-L".to_string()); // 跟随重定向
    }
    curl_command.push("--connect-timeout".to_string());
    curl_command.push(state.config.curl.timeout_seconds.to_string());
    curl_command.push("--max-filesize".to_string());
    curl_command.push(state.config.proxy.max_response_size_bytes.to_string());

    // 添加目标URL
    curl_command.push(target_url.to_string());

    let curl_command_str = curl_command.join(" ");
    info!("Executing curl command: {}", curl_command_str);

    // 执行curl命令
    let output = Command::new("curl")
        .args(&curl_command[1..]) // 去掉第一个"curl"
        .output()
        .map_err(|e| AppError::InternalServerError(format!("Failed to execute curl: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        warn!("Curl command failed: {}", stderr);
        
        // 检查是否是文件大小超限
        if stderr.contains("Maximum file size exceeded") {
            // 使用无大小限制的curl重新获取并上传到temp-file-host
            return upload_large_response(state, target_url, &curl_command_str, headers, method).await;
        }

        return Ok(CurlResponse {
            curl_command: curl_command_str,
            response_body: None,
            response_headers: None,
            status_code: None,
            error: Some(stderr.to_string()),
            redirected: false,
            redirect_url: None,
            uploaded_to_temp_host: false,
            temp_file_url: None,
        }
        .into_response());
    }

    // 解析响应
    let (response_headers, response_body) = if state.config.curl.include_headers {
        parse_curl_response(&stdout)
    } else {
        (HashMap::new(), stdout.to_string())
    };

    Ok(CurlResponse {
        curl_command: curl_command_str,
        response_body: Some(response_body),
        response_headers: Some(response_headers),
        status_code: None, // 可以从headers中解析
        error: None,
        redirected: false,
        redirect_url: None,
        uploaded_to_temp_host: false,
        temp_file_url: None,
    }
    .into_response())
}

pub async fn upload_large_response(
    state: &Arc<AppState>,
    target_url: &str,
    curl_command: &str,
    headers: &HeaderMap,
    method: &str,
) -> Result<Response, AppError> {
    info!("Response too large, fetching and uploading to temp-file-host");

    // 构建无大小限制的curl命令
    let mut unlimited_curl_command = vec!["curl".to_string()];
    
    if method != "GET" {
        unlimited_curl_command.push("-X".to_string());
        unlimited_curl_command.push(method.to_string());
    }

    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            unlimited_curl_command.push("-H".to_string());
            unlimited_curl_command.push(format!("{}: {}", name.as_str(), value_str));
        }
    }

    unlimited_curl_command.push("-s".to_string());
    if state.config.curl.follow_redirects {
        unlimited_curl_command.push("-L".to_string());
    }
    unlimited_curl_command.push("--connect-timeout".to_string());
    unlimited_curl_command.push(state.config.curl.timeout_seconds.to_string());
    unlimited_curl_command.push(target_url.to_string());

    // 执行curl获取完整响应
    let output = Command::new("curl")
        .args(&unlimited_curl_command[1..])
        .output()
        .map_err(|e| AppError::InternalServerError(format!("Failed to execute unlimited curl: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(CurlResponse {
            curl_command: curl_command.to_string(),
            response_body: None,
            response_headers: None,
            status_code: None,
            error: Some(format!("Failed to fetch large response: {}", stderr)),
            redirected: false,
            redirect_url: None,
            uploaded_to_temp_host: false,
            temp_file_url: None,
        }
        .into_response());
    }

    let response_data = output.stdout;
    
    // 上传到temp-file-host
    match upload_to_temp_host(state, &response_data).await {
        Ok(temp_url) => {
            info!("Successfully uploaded large response to temp-file-host: {}", temp_url);
            Ok(CurlResponse {
                curl_command: curl_command.to_string(),
                response_body: None,
                response_headers: None,
                status_code: None,
                error: None,
                redirected: false,
                redirect_url: None,
                uploaded_to_temp_host: true,
                temp_file_url: Some(temp_url),
            }
            .into_response())
        }
        Err(e) => {
            error!("Failed to upload to temp-file-host: {}", e);
            Ok(CurlResponse {
                curl_command: curl_command.to_string(),
                response_body: None,
                response_headers: None,
                status_code: None,
                error: Some(format!("Failed to upload large response: {}", e)),
                redirected: false,
                redirect_url: None,
                uploaded_to_temp_host: false,
                temp_file_url: None,
            }
            .into_response())
        }
    }
}

async fn upload_to_temp_host(
    state: &Arc<AppState>,
    data: &[u8],
) -> Result<String> {
    // 生成文件名
    let filename = utils_share::time::generate_timestamped_filename("curl_response", "dat");
    
    // 创建multipart表单
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(data.to_vec())
            .file_name(filename.clone())
            .mime_str("application/octet-stream")?);

    // 上传到temp-file-host
    let upload_url = format!("{}/upload", state.config.proxy.temp_file_host_url);
    let response = state.http_client
        .post(&upload_url)
        .multipart(form)
        .send()
        .await?;

    if response.status().is_success() {
        let download_url = response.text().await?;
        Ok(download_url)
    } else {
        Err(anyhow::anyhow!("Upload failed with status: {}", response.status()))
    }
} 