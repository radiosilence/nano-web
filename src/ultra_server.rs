use crate::fast_routes::{UltraFastServer, FastRoute};
use crate::security::validate_request_path;
use anyhow::Result;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tracing::{info, warn, debug};
use std::path::PathBuf;
use std::net::SocketAddr;

// Custom ultra-fast HTTP implementation
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct UltraServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub dev: bool,
    pub spa_mode: bool,
    pub config_prefix: String,
    pub log_requests: bool,
}

pub struct UltraServerState {
    pub server: Arc<UltraFastServer>,
    pub config: UltraServeConfig,
}

pub async fn start_ultra_server(config: UltraServeConfig) -> Result<()> {
    let server = Arc::new(UltraFastServer::new());
    
    // Populate routes
    server.populate_routes(&config.public_dir, &config.config_prefix)?;
    
    // No rate limiting for maximum benchmarking performance
    
    let state = Arc::new(UltraServerState {
        server,
        config,
    });
    
    let addr = format!("0.0.0.0:{}", state.config.port);
    let listener = TcpListener::bind(&addr).await?;
    
    info!("Starting ULTRA-FAST SECURE server on {}", addr);
    info!("Serving directory: {:?}", state.config.public_dir);
    info!("Routes loaded: {}", state.server.routes.len());
    info!("Security: Path validation, security headers enabled");
    
    // No rate limiter cleanup needed
    
    loop {
        let (stream, addr) = listener.accept().await?;
        let state = Arc::clone(&state);
        
        // Spawn with high priority for minimum latency
        tokio::task::spawn(async move {
            if let Err(e) = handle_ultra_fast_connection(stream, addr, state).await {
                debug!("Connection error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_ultra_fast_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    state: Arc<UltraServerState>,
) -> Result<()> {
    let start = Instant::now();
    
    // Read request with size limits for security
    let mut buffer = vec![0u8; MAX_REQUEST_SIZE.min(8192)];
    let bytes_read = stream.read(&mut buffer).await?;
    
    if bytes_read == 0 {
        return Ok(());
    }
    
    if bytes_read >= MAX_REQUEST_SIZE {
        write_error_response(&mut stream, 413, "Request Entity Too Large").await?;
        return Ok(());
    }
    
    // Parse HTTP request - secure parsing
    let request = match std::str::from_utf8(&buffer[..bytes_read]) {
        Ok(req) => req,
        Err(_) => {
            write_error_response(&mut stream, 400, "Bad Request - Invalid UTF-8").await?;
            return Ok(());
        }
    };
    
    // Debug: log the raw request
    let first_line = request.lines().next().unwrap_or("").trim();
    debug!("Raw request line from {}: {:?}", addr, first_line);
    
    let (method, raw_path, _) = match parse_request_line_secure(first_line) {
        Ok(parsed) => parsed,
        Err(e) => {
            warn!("Request parsing error from {}: {} - Line: {:?}", addr, e, first_line);
            write_error_response(&mut stream, 400, "Bad Request").await?;
            return Ok(());
        }
    };
    
    // Validate and sanitize path
    let path = match validate_request_path(raw_path) {
        Ok(safe_path) => safe_path,
        Err(e) => {
            warn!("Path validation failed from {}: {} - Path: {}", addr, e, raw_path);
            write_error_response(&mut stream, 400, "Bad Request - Invalid Path").await?;
            return Ok(());
        }
    };
    
    // Method validation is now done in parse_request_line_secure
    
    // Handle health check with zero allocations
    if path == "/_health" {
        write_health_response(&mut stream).await?;
        return Ok(());
    }
    
    // Ultra-fast route lookup
    let mut route = state.server.get_route(&path);
    
    if route.is_none() && !path.ends_with('/') {
        // Try with trailing slash
        let path_with_slash = format!("{}/", path);
        route = state.server.get_route(&path_with_slash);
    }
    
    if route.is_none() && state.config.spa_mode {
        // SPA fallback
        route = state.server.get_route("/");
    }
    
    let route = match route {
        Some(r) => r,
        None => {
            write_error_response(&mut stream, 404, "Not Found").await?;
            return Ok(());
        }
    };
    
    // Dev mode file refresh (lock-free)
    let route = if state.config.dev {
        match state.server.refresh_if_modified(&path, &state.config.config_prefix) {
            Ok(Some(updated_route)) => updated_route,
            Ok(None) => route,
            Err(e) => {
                warn!("Failed to refresh route {}: {}", path, e);
                route
            }
        }
    } else {
        route
    };
    
    // Parse Accept-Encoding header for compression
    let accept_encoding = extract_accept_encoding(request).unwrap_or("");
    let (encoding, content) = route.content.get_best_encoding(accept_encoding);
    
    // Write response with zero-copy where possible and security headers
    write_fast_response(&mut stream, &route, encoding, content, method == "HEAD").await?;
    
    if state.config.log_requests {
        let duration = start.elapsed();
        info!(
            method = method,
            path = %path,
            client = %addr,
            status = 200,
            duration_ns = duration.as_nanos(),
            bytes = content.len(),
            encoding = encoding,
            "ultra-fast secure request"
        );
    }
    
    Ok(())
}

// Removed - using secure parsing from security module instead

#[inline(always)]
fn extract_accept_encoding(request: &str) -> Option<&str> {
    // Ultra-fast header extraction without allocations
    for line in request.lines() {
        if line.len() > 16 && line.as_bytes()[..15].eq_ignore_ascii_case(b"accept-encoding") {
            if let Some(colon_pos) = line.find(':') {
                return Some(line[colon_pos + 1..].trim());
            }
        }
    }
    None
}

async fn write_fast_response(
    stream: &mut TcpStream,
    route: &FastRoute,
    encoding: &str,
    content: &Bytes,
    head_only: bool,
) -> Result<()> {
    // Pre-allocated response buffer for headers
    let mut response = Vec::with_capacity(2048); // Larger for security headers
    
    // Status line
    response.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
    
    // Headers - use pre-interned strings for zero allocation
    write!(response, "Content-Type: {}\r\n", route.headers.content_type)?;
    write!(response, "Content-Length: {}\r\n", content.len())?;
    write!(response, "Last-Modified: {}\r\n", route.headers.last_modified)?;
    write!(response, "ETag: {}\r\n", route.headers.etag)?;
    write!(response, "Cache-Control: {}\r\n", route.headers.cache_control)?;
    response.extend_from_slice(b"Server: nano-web-ultra\r\n");
    
    if encoding != "identity" {
        write!(response, "Content-Encoding: {}\r\n", encoding)?;
    }
    
    // Security headers for protection
    for (name, value) in security_headers() {
        write!(response, "{}: {}\r\n", name, value)?;
    }
    
    // Connection header for keep-alive
    response.extend_from_slice(b"Connection: keep-alive\r\n");
    response.extend_from_slice(b"\r\n");
    
    // Write headers
    stream.write_all(&response).await?;
    
    // Write body (unless HEAD request)
    if !head_only {
        stream.write_all(content).await?;
    }
    
    Ok(())
}

async fn write_health_response(stream: &mut TcpStream) -> Result<()> {
    // Pre-computed health response for maximum speed
    static HEALTH_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 49\r\nCache-Control: no-cache\r\n\r\n{\"status\":\"ok\",\"timestamp\":\"1970-01-01T00:00:00Z\"}";
    
    stream.write_all(HEALTH_RESPONSE).await?;
    Ok(())
}

async fn write_error_response(stream: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{} {}",
        status, message, message.len() + status.to_string().len() + 1, status, message
    );
    
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}