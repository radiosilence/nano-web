// io_uring based server implementation (Linux only)
// Zero-copy serving with registered buffers

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_uring::net::TcpListener;
use tracing::{debug, info, warn};

use crate::http::{build_response, parse_request};
use crate::registered_buffers::RegisteredBufferManager;
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

    // Pre-build all HTTP responses
    info!("Pre-building HTTP responses...");
    let buffer_manager = Arc::new(
        RegisteredBufferManager::new(&nano_web.routes).context("Failed to pre-build responses")?,
    );
    info!(
        "Pre-built {} response variants",
        buffer_manager.buffer_count()
    );

    // Create io_uring instance and register buffers with kernel
    info!("Registering buffers with io_uring...");
    let ring_builder = tokio_uring::uring_builder();
    let ring = ring_builder
        .build(256)
        .context("Failed to build io_uring")?;

    // Convert our Bytes buffers to iovec for registration
    let iovecs: Vec<libc::iovec> = buffer_manager
        .buffers()
        .iter()
        .map(|b| libc::iovec {
            iov_base: b.as_ptr() as *mut libc::c_void,
            iov_len: b.len(),
        })
        .collect();

    // Register buffers with the kernel
    unsafe {
        ring.submitter()
            .register_buffers(&iovecs)
            .context("Failed to register buffers with io_uring")?;
    }

    info!("Registered {} fixed buffers with kernel", iovecs.len());

    // Start io_uring runtime with our configured ring
    let mut builder = tokio_uring::builder();
    builder.uring_builder(&ring_builder);

    builder.start(async move {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse().unwrap();
        let listener = TcpListener::bind(addr)
            .context("Failed to bind to address")
            .unwrap();

        info!("Server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let buffer_manager = buffer_manager.clone();
                    let spa_mode = config.spa_mode;

                    // Spawn handler for this connection
                    tokio_uring::spawn(async move {
                        if let Err(e) =
                            handle_connection(stream, buffer_manager, spa_mode, peer_addr).await
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
    })
}

/// Handle a single connection (HTTP/1.1 keep-alive supported)
async fn handle_connection(
    stream: tokio_uring::net::TcpStream,
    buffer_manager: Arc<RegisteredBufferManager>,
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

        // Only support GET and HEAD
        if request.method != "GET" && request.method != "HEAD" {
            let response = build_response(405, &[], b"Method Not Allowed");
            let _ = write_all(&stream, &response).await;
            continue;
        }

        let _is_head = request.method == "HEAD";

        // Get Accept-Encoding header
        let accept_encoding = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-encoding"))
            .map(|(_, v)| *v)
            .unwrap_or("");

        let path = request.path;

        // Runtime logic: map[path][encoding]
        let response = if let Some((_, encoding)) = buffer_manager.best_match(path, accept_encoding)
        {
            buffer_manager.get(path, encoding).unwrap().data.to_vec()
        } else if spa_mode {
            // SPA fallback: if not found, serve /index.html
            if let Some((_, encoding)) = buffer_manager.best_match("/index.html", accept_encoding) {
                buffer_manager
                    .get("/index.html", encoding)
                    .unwrap()
                    .data
                    .to_vec()
            } else {
                build_response(404, &[], b"Not Found")
            }
        } else {
            build_response(404, &[], b"Not Found")
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

// Note: build_file_response removed - we now use pre-built responses from RegisteredBufferManager

/// Write all data to stream
async fn write_all(stream: &tokio_uring::net::TcpStream, data: &[u8]) -> Result<()> {
    let mut written = 0;
    let buf = data.to_vec();

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
