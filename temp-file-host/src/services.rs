use crate::models::AppError;
use chrono::Utc;
use filetime::{FileTime, set_file_mtime};
use futures_util::StreamExt;
use sha2::Digest;
use std::path::PathBuf;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{info, warn};

pub async fn cleanup_old_files(upload_dir: &PathBuf, days_threshold: i64) -> anyhow::Result<usize> {
    let mut deleted_count = 0;
    let now = Utc::now();
    let cutoff_time = now - chrono::Duration::days(days_threshold);

    let mut entries = fs::read_dir(upload_dir).await?;

    while let Some(entry_result) = entries.next_entry().await? {
        let entry_path = entry_result.path();
        if entry_path.is_file() {
            match fs::metadata(&entry_path).await {
                Ok(metadata) => {
                    if let Ok(modified_time) = metadata.modified() {
                        let modified_chrono: chrono::DateTime<Utc> = modified_time.into();
                        if modified_chrono < cutoff_time {
                            info!(
                                "Deleting old file: {:?} (modified: {})",
                                entry_path, modified_chrono
                            );
                            match fs::remove_file(&entry_path).await {
                                Ok(_) => deleted_count += 1,
                                Err(e) => warn!("Failed to delete file {:?}: {}", entry_path, e),
                            }
                        }
                    } else {
                        warn!("Could not get modified time for file: {:?}", entry_path);
                    }
                }
                Err(e) => {
                    warn!("Could not get metadata for file {:?}: {}", entry_path, e);
                }
            }
        }
    }
    Ok(deleted_count)
}

fn reset_file_mtime(path: &PathBuf) -> std::io::Result<()> {
    let now = FileTime::from_system_time(std::time::SystemTime::now());
    set_file_mtime(path, now)?;
    Ok(())
}

pub async fn save_file(
    state: &crate::config::AppState,
    field: axum::extract::multipart::Field<'_>,
) -> Result<(String, String), AppError> {
    if field.file_name().is_none() || field.file_name().unwrap().is_empty() {
        return Err(AppError::UserError("Filename is empty".to_string()));
    }

    let original_filename = field.file_name().unwrap().to_owned();
    info!("Receiving upload for file: {}", &original_filename);

    let mut hasher = sha2::Sha256::new();
    let mut total_size: usize = 0;

    // 创建临时文件
    let temp_path = state
        .upload_path
        .join(format!("{}.tmpupload", original_filename));
    let mut temp_file = fs::File::create(&temp_path).await?;

    // 将 axum Field 转换为异步流
    let mut stream = field.map(|chunk_result| {
        chunk_result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    // 流式处理数据
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        total_size += chunk.len();

        // 检查文件大小限制
        if total_size > state.max_file_size {
            warn!(
                "Upload aborted: File '{}' exceeded size limit of {} bytes",
                original_filename, state.max_file_size
            );
            // 清理临时文件
            let _ = fs::remove_file(&temp_path).await;
            return Err(AppError::PayloadTooLarge(format!(
                "File size exceeds limit of {} MB",
                state.max_file_size / 1024 / 1024
            )));
        }

        // 更新哈希
        hasher.update(&chunk);
        // 写入文件
        temp_file.write_all(&chunk).await?;
    }

    // 确保所有数据都写入磁盘
    temp_file.flush().await?;

    // 计算最终哈希值
    let sha256_hash_bytes = hasher.finalize();
    let sha256_hash_hex = hex::encode(sha256_hash_bytes);
    let short_hash = &sha256_hash_hex[..16]; // 只使用前16位
    let final_path = state.upload_path.join(short_hash);

    if final_path.exists() {
        info!(
            "File with hash {} already exists for '{}'",
            short_hash, original_filename
        );
        let _ = fs::remove_file(&temp_path).await;
        reset_file_mtime(&final_path)?;
    } else {
        info!(
            "Saving file '{}' ({:.2} MB) with hash {}",
            original_filename,
            total_size as f64 / 1024.0 / 1024.0,
            short_hash
        );
        fs::rename(&temp_path, &final_path).await?;
        let _ = fs::remove_file(&temp_path).await;
        info!("Successfully saved file to {:?}", final_path);
    }

    Ok((original_filename, short_hash.to_string()))
}

#[tokio::test]
async fn test_cleanup_old_files() {
    let upload_dir = PathBuf::from("./.temporary_uploads");
    let _ = fs::remove_dir_all(&upload_dir).await;
    let _ = fs::create_dir_all(&upload_dir).await;

    let file_path = upload_dir.join("test.txt");

    let _ = fs::File::create(&file_path).await;
    let one_week_ago =
        std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 24 * 60 * 60);
    filetime::set_file_mtime(&file_path, FileTime::from_system_time(one_week_ago)).unwrap();
    assert_eq!(0, cleanup_old_files(&upload_dir, 10).await.unwrap());
    assert_eq!(1, cleanup_old_files(&upload_dir, 1).await.unwrap());

    let _ = fs::File::create(&file_path).await;
    let one_week_ago =
        std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 24 * 60 * 60);
    filetime::set_file_mtime(&file_path, FileTime::from_system_time(one_week_ago)).unwrap();
    assert_eq!(1, cleanup_old_files(&upload_dir, 3).await.unwrap());
}
