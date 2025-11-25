use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{server::conn::http1, service::service_fn, Method, Request, Response, StatusCode};
use monoio::{io::IntoPollIo, net::TcpListener};
use monoio_compat::hyper::{MonoioIo, MonoioTimer};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::{debug, info};

use crate::response_buffer::ResponseBuffer;
use crate::routes::NanoWeb;

#[derive(Clone)]
pub struct ServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub dev: bool,
    pub spa_mode: bool,
    pub config_prefix: String,
    pub log_requests: bool,
}

struct AppState {
    server: Rc<NanoWeb>,
    config: ServeConfig,
}

pub fn start_server(config: ServeConfig) -> Result<()> {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    info!("Starting {} worker threads", cores);

    let mut threads = Vec::with_capacity(cores);

    for i in 0..cores {
        let config = config.clone();
        threads.push(std::thread::spawn(move || {
            monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
                .enable_timer()
                .build()
                .expect("Failed to build monoio runtime")
                .block_on(run_server(config, i));
        }));
    }

    for t in threads {
        let _ = t.join();
    }

    Ok(())
}

async fn run_server(config: ServeConfig, worker_id: usize) {
    let server = Rc::new(NanoWeb::new());
    if let Err(e) = server.populate_routes(&config.public_dir, &config.config_prefix) {
        tracing::error!("Failed to populate routes: {}", e);
        return;
    }

    if worker_id == 0 {
        info!("Routes loaded: {}", server.routes.len());
    }

    let state = Rc::new(AppState {
        server,
        config: config.clone(),
    });

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Worker {} failed to bind: {}", worker_id, e);
            return;
        }
    };

    if worker_id == 0 {
        info!("Starting server on http://{}", addr);
        info!("Serving directory: {:?}", config.public_dir);
    }

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                debug!("Accept error: {}", e);
                continue;
            }
        };

        let poll_stream = match stream.into_poll_io() {
            Ok(s) => MonoioIo::new(s),
            Err(e) => {
                debug!("Stream conversion error: {}", e);
                continue;
            }
        };

        let state = state.clone();
        monoio::spawn(async move {
            let service = service_fn(|req| {
                let state = state.clone();
                async move { handle_request(req, state).await }
            });

            if let Err(e) = http1::Builder::new()
                .timer(MonoioTimer)
                .keep_alive(true)
                .pipeline_flush(true)
                .serve_connection(poll_stream, service)
                .await
            {
                debug!("Connection error: {}", e);
            }
        });
    }
}

type HyperResponse = Response<Full<Bytes>>;

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Rc<AppState>,
) -> Result<HyperResponse, std::convert::Infallible> {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return Ok(response(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
        ));
    }

    let path = req.uri().path();

    // Health check
    if path == "/_health" {
        let body = format!(
            r#"{{"status":"ok","timestamp":"{}"}}"#,
            chrono::Utc::now().to_rfc3339()
        );
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body)))
            .unwrap());
    }

    // Path validation
    if let Err(e) = crate::path::validate_request_path(path) {
        tracing::warn!("Path validation failed for '{}': {}", path, e);
        return Ok(response(StatusCode::BAD_REQUEST, "Bad Request"));
    }

    // Dev mode: refresh if modified
    if state.config.dev {
        let _ = state.server.refresh_if_modified(
            path,
            &state.config.public_dir,
            &state.config.config_prefix,
        );
    }

    let accept_encoding = req
        .headers()
        .get("accept-encoding")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let mut buf = state.server.get_response(path, accept_encoding);

    // Try with trailing slash
    if buf.is_none() && !path.ends_with('/') {
        let with_slash = format!("{}/", path);
        buf = state.server.get_response(&with_slash, accept_encoding);
    }

    // SPA fallback
    if buf.is_none() && state.config.spa_mode {
        debug!("SPA fallback for: {}", path);
        buf = state.server.get_response("/", accept_encoding);
    }

    match buf {
        Some(b) => Ok(build_response(&b)),
        None => {
            debug!("Route not found: {}", path);
            Ok(response(StatusCode::NOT_FOUND, "Not Found"))
        }
    }
}

#[inline(always)]
fn response(status: StatusCode, body: &'static str) -> HyperResponse {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}

#[inline(always)]
fn build_response(buf: &ResponseBuffer) -> HyperResponse {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", buf.content_type.as_ref())
        .header("etag", buf.etag.as_ref())
        .header("last-modified", buf.last_modified.as_ref())
        .header("cache-control", buf.cache_control.as_ref())
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "SAMEORIGIN")
        .header("referrer-policy", "strict-origin-when-cross-origin");

    if let Some(encoding) = buf.content_encoding {
        builder = builder.header("content-encoding", encoding);
    }

    builder.body(Full::new(buf.body.clone())).unwrap()
}
