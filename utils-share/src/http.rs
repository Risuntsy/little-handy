use std::collections::HashMap;

pub fn parse_curl_response(output: &str) -> (HashMap<String, String>, String) {
    let mut headers = HashMap::new();
    let mut body = String::new();
    let mut in_headers = true;

    for line in output.lines() {
        if in_headers {
            if line.trim().is_empty() {
                in_headers = false;
                continue;
            }
            
            if line.starts_with("HTTP/") {
                headers.insert("status".to_string(), line.to_string());
            } else if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim();
                let value = line[colon_pos + 1..].trim();
                headers.insert(name.to_lowercase(), value.to_string());
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    (headers, body.trim_end().to_string())
}

pub fn validate_hash_format(hash: &str) -> bool {
    hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn sanitize_filename(filename: &str) -> String {
    filename.replace(['/', '\\'], "_")
} 