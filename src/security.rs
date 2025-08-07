use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// Maximum request size to prevent DoS attacks (16KB)
pub const MAX_REQUEST_SIZE: usize = 16 * 1024;

/// Maximum path length to prevent buffer overflows
pub const MAX_PATH_LENGTH: usize = 1024;

/// Maximum number of path components to prevent deeply nested attacks
pub const MAX_PATH_COMPONENTS: usize = 32;

/// Validate and sanitize HTTP request path to prevent directory traversal attacks
pub fn validate_request_path(path: &str) -> Result<String> {
    // Basic length check
    if path.len() > MAX_PATH_LENGTH {
        bail!("Path too long");
    }
    
    // Must start with /
    if !path.starts_with('/') {
        bail!("Path must start with /");
    }
    
    // Decode URL encoding safely
    let decoded = match urlencoding::decode(path) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => bail!("Invalid URL encoding"),
    };
    
    // Check for null bytes (security)
    if decoded.contains('\0') {
        bail!("Path contains null bytes");
    }
    
    // Split into components and validate each
    let components: Vec<&str> = decoded.split('/').skip(1).collect(); // Skip first empty component
    
    if components.len() > MAX_PATH_COMPONENTS {
        bail!("Too many path components");
    }
    
    let mut sanitized_components = Vec::new();
    
    for component in components {
        // Skip empty components (double slashes)
        if component.is_empty() {
            continue;
        }
        
        // Reject dangerous components
        if component == ".." || component == "." {
            bail!("Path traversal attempt detected");
        }
        
        // Check for dangerous characters
        if component.contains(['\\', '\0', '<', '>', '|', '?', '*']) {
            bail!("Invalid characters in path component");
        }
        
        // Reject hidden files/directories (starting with .)
        if component.starts_with('.') {
            bail!("Access to hidden files denied");
        }
        
        // Reject overly long components
        if component.len() > 255 {
            bail!("Path component too long");
        }
        
        sanitized_components.push(component);
    }
    
    // Reconstruct safe path
    let safe_path = if sanitized_components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", sanitized_components.join("/"))
    };
    
    Ok(safe_path)
}

/// Validate that a file path is within the allowed public directory
pub fn validate_file_path(file_path: &Path, public_dir: &Path) -> Result<()> {
    // Canonicalize paths to resolve any symlinks or relative components
    let canonical_file = file_path.canonicalize()
        .map_err(|_| anyhow::anyhow!("File path canonicalization failed"))?;
    
    let canonical_public = public_dir.canonicalize()
        .map_err(|_| anyhow::anyhow!("Public directory canonicalization failed"))?;
    
    // Ensure the file is within the public directory
    if !canonical_file.starts_with(&canonical_public) {
        bail!("File path outside public directory");
    }
    
    Ok(())
}

/// Parse HTTP request line with strict validation
pub fn parse_request_line_secure(line: &str) -> Result<(&str, &str, &str)> {
    // Basic line validation
    if line.len() > 4096 {
        bail!("Request line too long");
    }
    
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    
    if parts.len() != 3 {
        bail!("Invalid HTTP request line format");
    }
    
    let method = parts[0];
    let path = parts[1];
    let version = parts[2];
    
    // Validate HTTP method (allow only safe methods)
    match method {
        "GET" | "HEAD" | "OPTIONS" => {},
        _ => bail!("Method not allowed"),
    }
    
    // Validate HTTP version
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        bail!("Unsupported HTTP version");
    }
    
    // Basic path validation
    if path.len() > MAX_PATH_LENGTH {
        bail!("Request path too long");
    }
    
    Ok((method, path, version))
}

/// Validate HTTP headers for security
pub fn validate_http_header(name: &str, value: &str) -> Result<()> {
    // Header name validation
    if name.is_empty() || name.len() > 256 {
        bail!("Invalid header name length");
    }
    
    // Check for valid header name characters (RFC 7230)
    if !name.chars().all(|c| c.is_ascii() && (c.is_alphanumeric() || "-_".contains(c))) {
        bail!("Invalid header name characters");
    }
    
    // Header value validation
    if value.len() > 8192 {
        bail!("Header value too long");
    }
    
    // Check for control characters (except HT, SP)
    if value.chars().any(|c| c.is_control() && c != '\t' && c != ' ') {
        bail!("Invalid header value characters");
    }
    
    Ok(())
}

/// Rate limiting structure (simple token bucket)
pub struct RateLimiter {
    max_requests: u32,
    window_seconds: u64,
    requests: dashmap::DashMap<std::net::IpAddr, (std::time::Instant, u32)>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
            requests: dashmap::DashMap::new(),
        }
    }
    
    pub fn is_allowed(&self, ip: std::net::IpAddr) -> bool {
        let now = std::time::Instant::now();
        let window_duration = std::time::Duration::from_secs(self.window_seconds);
        
        let mut entry = self.requests.entry(ip).or_insert((now, 0));
        let (last_reset, count) = entry.value_mut();
        
        // Reset window if expired
        if now.duration_since(*last_reset) > window_duration {
            *last_reset = now;
            *count = 0;
        }
        
        // Check if under limit
        if *count >= self.max_requests {
            false
        } else {
            *count += 1;
            true
        }
    }
    
    /// Cleanup old entries periodically
    pub fn cleanup(&self) {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(self.window_seconds * 2);
        
        self.requests.retain(|_, (last_reset, _)| *last_reset > cutoff);
    }
}

/// Content Security Policy headers for enhanced security
pub fn security_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("X-XSS-Protection", "1; mode=block"),
        ("Referrer-Policy", "strict-origin-when-cross-origin"),
        ("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';"),
        ("Strict-Transport-Security", "max-age=31536000; includeSubDomains"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_validation() {
        // Valid paths
        assert!(validate_request_path("/").is_ok());
        assert!(validate_request_path("/index.html").is_ok());
        assert!(validate_request_path("/assets/style.css").is_ok());
        
        // Invalid paths
        assert!(validate_request_path("../etc/passwd").is_err());
        assert!(validate_request_path("/.env").is_err());
        assert!(validate_request_path("/path/with/../../traversal").is_err());
        assert!(validate_request_path("/path\0null").is_err());
    }
    
    #[test]
    fn test_request_line_parsing() {
        assert!(parse_request_line_secure("GET / HTTP/1.1").is_ok());
        assert!(parse_request_line_secure("HEAD /test HTTP/1.0").is_ok());
        assert!(parse_request_line_secure("OPTIONS * HTTP/1.1").is_ok());
        
        // Invalid requests
        assert!(parse_request_line_secure("POST / HTTP/1.1").is_err());
        assert!(parse_request_line_secure("GET /").is_err());
        assert!(parse_request_line_secure("INVALID REQUEST").is_err());
    }
}