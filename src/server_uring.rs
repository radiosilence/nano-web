// io_uring based server implementation (Linux only)
// Zero-copy serving with registered buffers

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_uring::net::TcpListener;
use tracing::{debug, info, warn};

use crate::compression::CompressedContent;
use crate::http::{build_response, parse_request};
use crate::routes::NanoWeb;

pub struct UringServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub dev: bool,
    pub spa_mode: bool,
    pub config_prefix: String,
}

/// Start io_uring-based server
pub fn serve(config: UringServeConfig) -> Result<()> {
    info!("Starting io_uring server on 0.0.0.0:{}", config.port);
    info!("Pre-loading files from {:?}", config.public_dir);

    // Pre-load all files into memory
    let nano_web = Arc::new(NanoWeb::new());
    nano_web
        .populate_routes(&config.public_dir, &config.config_prefix)
        .context("Failed to populate routes")?;

    info!("Routes loaded: {}", nano_web.routes.len());

    // Start io_uring runtime
    tokio_uring::start(async move {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();
        let listener = TcpListener::bind(addr)
            .context("Failed to bind to address")
            .unwrap();

        info!("Server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let nano_web = nano_web.clone();
                    let spa_mode = config.spa_mode;

                    // Spawn handler for this connection
                    tokio_uring::spawn(async move {
                        if let Err(e) =
                            handle_connection(stream, nano_web, spa_mode, peer_addr).await
                        {
                            warn!("Connection error from {}: {:?}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    warn!("Accept error: {:?}", e);
                }
            }
        }
    });

    Ok(())
}

/// Handle a single connection (HTTP/1.1 keep-alive supported)
async fn handle_connection(
    stream: tokio_uring::net::TcpStream,
    nano_web: Arc<NanoWeb>,
    spa_mode: bool,
    peer_addr: SocketAddr,
) -> Result<()> {
    debug!("Connection from {}", peer_addr);

    let mut buf = vec![0u8; 8192];
    let mut keep_alive = true;

    while keep_alive {
        // Read request
        let (result, nbuf) = stream.read(buf).await;
        buf = nbuf;

        let bytes_read = result.context("Failed to read from socket")?;
        if bytes_read == 0 {
            // Connection closed
            break;
        }

        // Parse HTTP request
        let (request, _body_offset) = match parse_request(&buf[..bytes_read]) {
            Ok(parsed) => parsed,
            Err(e) => {
                warn!("Parse error from {}: {:?}", peer_addr, e);
                // Send 400 Bad Request
                let response = build_response(400, &[], b"Bad Request");
                let _ = write_all(&stream, &response).await;
                break;
            }
        };

        // Check Connection header for keep-alive
        keep_alive = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("connection"))
            .map(|(_, v)| v.eq_ignore_ascii_case("keep-alive"))
            .unwrap_or(false);

        debug!("{} {} from {}", request.method, request.path, peer_addr);

        // Only support GET
        if request.method != "GET" {
            let response = build_response(405, &[], b"Method Not Allowed");
            let _ = write_all(&stream, &response).await;
            continue;
        }

        // Look up route
        let path = request.path;
        let response = match nano_web.routes.get(path) {
            Some(entry) => {
                // Found the file
                build_file_response(&entry.value().content, &entry.value().headers.content_type)
            }
            None => {
                // Try index.html for directories
                let index_path = if path.ends_with('/') {
                    format!("{}index.html", path)
                } else {
                    format!("{}/index.html", path)
                };
                let index_path_str = index_path.as_str();

                match nano_web.routes.get(index_path_str) {
                    Some(entry) => build_file_response(
                        &entry.value().content,
                        &entry.value().headers.content_type,
                    ),
                    None => {
                        // SPA mode fallback
                        if spa_mode {
                            match nano_web.routes.get("/index.html") {
                                Some(entry) => build_file_response(
                                    &entry.value().content,
                                    &entry.value().headers.content_type,
                                ),
                                None => build_response(404, &[], b"Not Found"),
                            }
                        } else {
                            build_response(404, &[], b"Not Found")
                        }
                    }
                }
            }
        };

        // Write response
        if let Err(e) = write_all(&stream, &response).await {
            warn!("Write error to {}: {:?}", peer_addr, e);
            break;
        }
    }

    debug!("Connection closed from {}", peer_addr);
    Ok(())
}

/// Build HTTP response for a file
fn build_file_response(content: &Arc<CompressedContent>, content_type: &Arc<str>) -> Vec<u8> {
    // Use the uncompressed content for now (io_uring with registered buffers would use this)
    let body = &content.original;

    let headers = [
        ("Content-Type", content_type.as_ref()),
        ("Cache-Control", "public, max-age=3600"),
        ("Server", "nano-web-uring"),
    ];

    build_response(200, &headers, body)
}

/// Write all data to stream
async fn write_all(stream: &tokio_uring::net::TcpStream, data: &[u8]) -> Result<()> {
    let mut written = 0;
    let mut buf = data.to_vec();

    while written < data.len() {
        let slice = buf[written..].to_vec();
        let (result, _) = stream.write(slice).submit().await;
        let n = result.context("Write failed")?;
        if n == 0 {
            anyhow::bail!("Connection closed while writing");
        }
        written += n;
    }

    Ok(())
}
