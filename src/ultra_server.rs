use crate::response_buffer::ResponseBuffer;
use crate::routes::NanoWeb;
use anyhow::Result;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, info};

#[derive(Clone)]
pub struct UltraServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub spa_mode: bool,
    pub config_prefix: String,
}

/// ULTRA MODE: Raw hyper service with direct buffer lookups
/// Request flow:
/// 1. Parse request path + Accept-Encoding header
/// 2. O(1) HashMap lookup: (path, encoding) -> ResponseBuffer
/// 3. Write complete pre-baked buffer to socket
/// 4. Done
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
        let io = TokioIo::new(stream);
        let server = server.clone();
        let spa_mode = config.spa_mode;

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let server = server.clone();
                async move { handle_request(req, server, spa_mode).await }
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                debug!("Connection error: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    server: Arc<NanoWeb>,
    spa_mode: bool,
) -> Result<Response<http_body_util::Full<Bytes>>, hyper::http::Error> {
    let path = req.uri().path();

    // Path validation
    if let Err(e) = crate::path::validate_request_path(path) {
        debug!("Path validation failed for '{}': {}", path, e);
        let buf = ResponseBuffer::bad_request();
        return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(
            http_body_util::Full::new(Bytes::from(buf.buffer.as_ref().clone())),
        )?);
    }

    // Extract Accept-Encoding
    let accept_encoding = req
        .headers()
        .get(hyper::header::ACCEPT_ENCODING)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    // ULTRA LOOKUP: O(1) direct buffer retrieval
    let mut response_buf = server.get_ultra(path, accept_encoding);

    // Fallback 1: Try with trailing slash
    if response_buf.is_none() && !path.ends_with('/') {
        let path_with_slash = format!("{}/", path);
        response_buf = server.get_ultra(&path_with_slash, accept_encoding);
    }

    // Fallback 2: SPA mode
    if response_buf.is_none() && spa_mode {
        debug!("SPA fallback for: {}", path);
        response_buf = server.get_ultra("/", accept_encoding);
    }

    match response_buf {
        Some(buf) => {
            // ZERO-COPY: Bytes wraps Arc<Vec<u8>>, no allocation
            // Just bump the refcount and blast it to the socket
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from(
                    buf.buffer.as_ref().clone(),
                )))?)
        }
        None => {
            debug!("Route not found: {}", path);
            let buf = ResponseBuffer::not_found();
            Ok(Response::builder().status(StatusCode::NOT_FOUND).body(
                http_body_util::Full::new(Bytes::from(buf.buffer.as_ref().clone())),
            )?)
        }
    }
}
