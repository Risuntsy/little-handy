use chrono::{DateTime, Utc};

pub fn generate_timestamped_filename(prefix: &str, extension: &str) -> String {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    format!("{}_{}.{}", prefix, timestamp, extension)
}

pub fn system_time_to_utc(system_time: std::time::SystemTime) -> DateTime<Utc> {
    system_time.into()
}

pub fn is_expired(timestamp: DateTime<Utc>, days_threshold: i64) -> bool {
    let now = Utc::now();
    let cutoff_time = now - chrono::Duration::days(days_threshold);
    timestamp < cutoff_time
}

pub fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    
    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        format!("{}m{}s", total_seconds / 60, total_seconds % 60)
    } else if total_seconds < 86400 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h{}m", hours, minutes)
    } else {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        format!("{}d{}h", days, hours)
    }
} 