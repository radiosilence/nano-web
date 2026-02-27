use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, info};

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
    server: Arc<NanoWeb>,
    config: ServeConfig,
}

/// Create a TCP listener with `SO_REUSEPORT` for better multi-core scaling
fn create_reuse_port_listener(addr: SocketAddr) -> Result<std::net::TcpListener> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(8192)?; // Large backlog for high concurrency
    Ok(socket.into())
}

pub async fn start_server(config: ServeConfig) -> Result<()> {
    let server = Arc::new(NanoWeb::new());
    server.populate_routes(&config.public_dir, &config.config_prefix)?;

    let state = Arc::new(AppState {
        server,
        config: config.clone(),
    });

    info!("Routes loaded: {}", state.server.route_count());

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let std_listener = create_reuse_port_listener(addr)?;
    let listener = TcpListener::from_std(std_listener)?;

    info!("Starting server on http://{}", addr);
    info!("Serving directory: {:?}", config.public_dir);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                async move { handle_request(req, state) }
            });

            if let Err(e) = http1::Builder::new()
                .keep_alive(true)
                .pipeline_flush(true)
                .serve_connection(io, service)
                .await
            {
                debug!("Connection error: {}", e);
            }
        });
    }
}

type HyperResponse = Response<Full<Bytes>>;

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<HyperResponse, std::convert::Infallible> {
    let is_head = req.method() == Method::HEAD;

    if req.method() != Method::GET && !is_head {
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
            httpdate::fmt_http_date(std::time::SystemTime::now())
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

    let if_none_match = req
        .headers()
        .get("if-none-match")
        .and_then(|h| h.to_str().ok());

    let mut buf = state.server.get_response(path, accept_encoding);

    // Try with trailing slash
    if buf.is_none() && !path.ends_with('/') {
        let with_slash = format!("{path}/");
        buf = state.server.get_response(&with_slash, accept_encoding);
    }

    // SPA fallback
    if buf.is_none() && state.config.spa_mode {
        debug!("SPA fallback for: {}", path);
        buf = state.server.get_response("/", accept_encoding);
    }

    let resp = if let Some(ref b) = buf {
        // ETag conditional: return 304 if client already has this version
        if let Some(etag) = if_none_match {
            if etag == b.etag.as_ref() {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header("etag", b.etag.as_ref())
                    .header("cache-control", b.cache_control.as_ref())
                    .body(Full::new(Bytes::new()))
                    .unwrap());
            }
        }
        build_response(b, is_head)
    } else {
        debug!("Route not found: {path}");
        response(StatusCode::NOT_FOUND, "Not Found")
    };

    if state.config.log_requests {
        info!(
            method = %req.method(),
            path = path,
            status = resp.status().as_u16(),
            "request"
        );
    }

    Ok(resp)
}

#[inline]
fn response(status: StatusCode, body: &'static str) -> HyperResponse {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}

#[inline]
fn build_response(buf: &crate::response_buffer::ResponseBuffer, head_only: bool) -> HyperResponse {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", buf.content_type.as_ref())
        .header("etag", buf.etag.as_ref())
        .header("last-modified", buf.last_modified.as_ref())
        .header("cache-control", buf.cache_control.as_ref())
        .header("x-content-type-options", "nosniff")
        .header("x-frame-options", "SAMEORIGIN")
        .header("referrer-policy", "strict-origin-when-cross-origin")
        .header(
            "strict-transport-security",
            "max-age=63072000; includeSubDomains",
        )
        .header(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        )
        .header("x-dns-prefetch-control", "off");

    if let Some(encoding) = buf.content_encoding {
        builder = builder.header("content-encoding", encoding);
    }

    if buf.content_encoding.is_some() {
        builder = builder.header("vary", "Accept-Encoding");
    }

    // Content-Length reflects the real body size even for HEAD (RFC 9110 §9.3.2)
    builder = builder.header("content-length", buf.body.len());

    let body = if head_only {
        Bytes::new()
    } else {
        buf.body.clone()
    };

    builder.body(Full::new(body)).unwrap()
}
