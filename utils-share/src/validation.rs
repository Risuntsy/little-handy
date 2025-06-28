use uuid::Uuid;

pub fn validate_uuid(uuid_str: &str) -> Result<Uuid, uuid::Error> {
    Uuid::parse_str(uuid_str)
}

pub fn validate_file_size(size: usize, max_size: usize) -> bool {
    size <= max_size
}

pub fn validate_file_extension(filename: &str, allowed_extensions: &[&str]) -> bool {
    filename
        .split('.')
        .last()
        .is_some_and(|extension| {
            allowed_extensions.contains(&extension.to_lowercase().as_str())
        })
}

pub fn validate_url(url: &str) -> bool {
    url::Url::parse(url).is_ok()
}

pub fn is_empty_or_whitespace(s: &str) -> bool {
    s.trim().is_empty()
}

pub fn validate_string_length(s: &str, min_len: usize, max_len: usize) -> bool {
    let len = s.len();
    len >= min_len && len <= max_len
}

pub fn extract_bearer_token(authorization: &str) -> Option<&str> {
    authorization.starts_with("Bearer ").then(|| &authorization[7..])
}
