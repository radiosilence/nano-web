use crate::response_buffer::ResponseBuffer;
use crate::routes::NanoWeb;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

#[derive(Clone)]
pub struct UltraServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub spa_mode: bool,
    pub config_prefix: String,
}

/// ULTRA MODE: Raw TCP with minimal HTTP parsing and pre-baked response buffers
/// Request flow:
/// 1. Accept TCP connection
/// 2. Parse minimal HTTP (GET /path HTTP/1.1 + Accept-Encoding header)
/// 3. O(1) HashMap lookup: (path, encoding) -> ResponseBuffer
/// 4. write_all() the complete pre-baked buffer to socket
/// 5. Done - zero allocations, zero parsing overhead
pub async fn start_ultra_server(config: UltraServeConfig) -> Result<()> {
    let server = Arc::new(NanoWeb::new());
    server.populate_routes(&config.public_dir, &config.config_prefix)?;

    info!("ULTRA MODE: Routes loaded: {}", server.routes.len());
    info!(
        "ULTRA MODE: Pre-baked responses: {}",
        server.ultra_cache.len()
    );

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;

    info!("ULTRA MODE: Server starting on http://{}", addr);
    info!("Serving directory: {:?}", config.public_dir);

    // Accept connections in a loop
    loop {
        let (stream, _) = listener.accept().await?;
        let server = server.clone();
        let spa_mode = config.spa_mode;

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, server, spa_mode).await {
                debug!("Connection error: {:?}", e);
            }
        });
    }
}

/// Handle raw TCP connection - minimal HTTP parsing
async fn handle_connection(
    mut stream: TcpStream,
    server: Arc<NanoWeb>,
    spa_mode: bool,
) -> Result<()> {
    let mut reader = BufReader::new(&mut stream);

    // Read request line: "GET /path HTTP/1.1"
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "GET" {
        // Only support GET requests
        let buf = ResponseBuffer::bad_request();
        stream.write_all(&buf.buffer).await?;
        stream.flush().await?;
        return Ok(());
    }

    let path = parts[1];

    // Path validation
    if let Err(e) = crate::path::validate_request_path(path) {
        debug!("Path validation failed for '{}': {}", path, e);
        let buf = ResponseBuffer::bad_request();
        stream.write_all(&buf.buffer).await?;
        stream.flush().await?;
        return Ok(());
    }

    // Read headers to find Accept-Encoding
    let mut accept_encoding = String::new();
    loop {
        let mut header_line = String::new();
        reader.read_line(&mut header_line).await?;

        // Empty line means end of headers
        if header_line == "\r\n" || header_line == "\n" {
            break;
        }

        // Check for Accept-Encoding header (case-insensitive)
        if header_line.to_lowercase().starts_with("accept-encoding:") {
            accept_encoding = header_line
                .split(':')
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
        }
    }

    // ULTRA LOOKUP: O(1) direct buffer retrieval
    let mut response_buf = server.get_ultra(path, &accept_encoding);

    // Fallback 1: Try with trailing slash
    if response_buf.is_none() && !path.ends_with('/') {
        let path_with_slash = format!("{}/", path);
        response_buf = server.get_ultra(&path_with_slash, &accept_encoding);
    }

    // Fallback 2: SPA mode
    if response_buf.is_none() && spa_mode {
        debug!("SPA fallback for: {}", path);
        response_buf = server.get_ultra("/", &accept_encoding);
    }

    match response_buf {
        Some(buf) => {
            // ZERO-COPY: Just write the Arc<Vec<u8>> buffer to socket
            // Complete HTTP response already in buffer
            stream.write_all(&buf.buffer).await?;
        }
        None => {
            debug!("Route not found: {}", path);
            let buf = ResponseBuffer::not_found();
            stream.write_all(&buf.buffer).await?;
        }
    }

    stream.flush().await?;
    Ok(())
}
