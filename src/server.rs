use anyhow::Result;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing::{debug, info};

use crate::routes::NanoWeb;

#[derive(Clone)]
pub struct AxumServeConfig {
    pub public_dir: PathBuf,
    pub port: u16,
    pub dev: bool,
    pub spa_mode: bool,
    pub config_prefix: String,
    pub log_requests: bool,
}

#[derive(Clone)]
struct AppState {
    server: Arc<NanoWeb>,
    config: AxumServeConfig,
}

pub async fn start_axum_server(config: AxumServeConfig) -> Result<()> {
    let server = Arc::new(NanoWeb::new());
    server.populate_routes(&config.public_dir, &config.config_prefix)?;

    let state = AppState {
        server,
        config: config.clone(),
    };

    info!("Routes loaded: {}", state.server.routes.len());

    let app = create_router(state);
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;

    info!("Starting server on http://{}", addr);
    info!("Serving directory: {:?}", config.public_dir);

    axum::serve(listener, app).await?;
    Ok(())
}

fn create_router(state: AppState) -> Router {
    let middleware_stack = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            "nosniff".parse::<axum::http::HeaderValue>().unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            "SAMEORIGIN".parse::<axum::http::HeaderValue>().unwrap(),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            "strict-origin-when-cross-origin"
                .parse::<axum::http::HeaderValue>()
                .unwrap(),
        ));

    let app = Router::new()
        .route("/_health", get(health_handler))
        .fallback(get(file_handler));

    if state.config.log_requests {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::extract::Request| {
                    tracing::info_span!("request", method = %request.method(), path = %request.uri().path())
                })
                .on_response(
                    |response: &axum::response::Response, latency: std::time::Duration, _span: &tracing::Span| {
                        tracing::info!(status = %response.status(), duration_ms = %latency.as_millis(), "request completed");
                    },
                ),
        )
        .layer(middleware_stack)
        .with_state(state)
    } else {
        app.layer(middleware_stack).with_state(state)
    }
}

async fn health_handler() -> impl IntoResponse {
    let response = format!(
        r#"{{"status":"ok","timestamp":"{}"}}"#,
        Utc::now().to_rfc3339()
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        response,
    )
}

async fn file_handler(
    uri: Uri,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    serve_file(uri.path(), headers, state).await
}

async fn serve_file(path: &str, request_headers: HeaderMap, state: AppState) -> impl IntoResponse {
    debug!("Serving path: {}", path);

    if let Err(e) = crate::path::validate_request_path(path) {
        tracing::warn!("Path validation failed for '{}': {}", path, e);
        return (StatusCode::BAD_REQUEST, "Bad Request").into_response();
    }

    // Dev mode: check if file changed and reload
    if state.config.dev {
        let _ = state.server.refresh_if_modified(
            path,
            &state.config.public_dir,
            &state.config.config_prefix,
        );
    }

    let accept_encoding = request_headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let mut response_buf = state.server.get_response(path, accept_encoding);

    // Try with trailing slash
    if response_buf.is_none() && !path.ends_with('/') {
        response_buf = state
            .server
            .get_response(&format!("{}/", path), accept_encoding);
    }

    // SPA fallback
    if response_buf.is_none() && state.config.spa_mode {
        debug!("SPA fallback for: {}", path);
        response_buf = state.server.get_response("/", accept_encoding);
    }

    match response_buf {
        Some(buf) => build_response(&buf).into_response(),
        None => {
            debug!("Route not found: {}", path);
            (StatusCode::NOT_FOUND, "Not Found").into_response()
        }
    }
}

fn build_response(buf: &crate::response_buffer::ResponseBuffer) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        buf.content_type.as_ref().parse().unwrap(),
    );
    headers.insert(header::ETAG, buf.etag.as_ref().parse().unwrap());
    headers.insert(
        header::LAST_MODIFIED,
        buf.last_modified.as_ref().parse().unwrap(),
    );
    headers.insert(
        header::CACHE_CONTROL,
        buf.cache_control.as_ref().parse().unwrap(),
    );

    if let Some(encoding) = buf.content_encoding {
        headers.insert(header::CONTENT_ENCODING, encoding.parse().unwrap());
    }

    (StatusCode::OK, headers, buf.body.clone()).into_response()
}
