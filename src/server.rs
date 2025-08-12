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

    // Populate routes using our existing route system
    server.populate_routes(&config.public_dir, &config.config_prefix)?;

    let state = AppState {
        server,
        config: config.clone(),
    };

    info!("Routes loaded: {}", state.server.routes.len());

    let app = create_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;

    info!("Starting server on {}", addr);
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
        .route("/", get(root_handler))
        .fallback(get(file_handler));

    if state.config.log_requests {
        app.layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::extract::Request| {
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        path = %request.uri().path(),
                    )
                })
                .on_response(|response: &axum::response::Response, latency: std::time::Duration, _span: &tracing::Span| {
                    tracing::info!(
                        status = %response.status(),
                        duration_ms = %latency.as_millis(),
                        "request completed"
                    );
                }),
        )
        .layer(middleware_stack)
        .with_state(state)
    } else {
        app.layer(middleware_stack).with_state(state)
    }
}

async fn health_handler() -> impl IntoResponse {
    let timestamp = Utc::now().to_rfc3339();
    let response = format!(r#"{{"status":"ok","timestamp":"{}"}}"#, timestamp);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        response,
    )
}

async fn root_handler(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    serve_file("/".to_string(), headers, state).await
}

async fn file_handler(
    uri: Uri,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    serve_file(path, headers, state).await
}

async fn serve_file(
    path: String,
    request_headers: HeaderMap,
    state: AppState,
) -> impl IntoResponse {
    debug!("Serving path: {}", path);

    // Security: validate path
    if let Err(e) = crate::path::validate_request_path(&path) {
        tracing::warn!("Path validation failed for '{}': {}", path, e);
        return (StatusCode::BAD_REQUEST, "Bad Request").into_response();
    }

    // Route lookup using our existing system
    let mut route = state.server.get_route(&path);

    if route.is_none() && !path.ends_with('/') {
        // Try with trailing slash
        let path_with_slash = format!("{}/", path);
        route = state.server.get_route(&path_with_slash);
    }

    if route.is_none() && state.config.spa_mode {
        // SPA fallback
        route = state.server.get_route("/");
        if route.is_some() {
            debug!("SPA fallback for: {}", path);
        }
    }

    let route = match route {
        Some(r) => r,
        None => {
            debug!("Route not found: {}", path);
            return (StatusCode::NOT_FOUND, "Not Found").into_response();
        }
    };

    // Dev mode file refresh
    let route = if state.config.dev {
        match state
            .server
            .refresh_if_modified(&path, &state.config.config_prefix)
        {
            Ok(Some(updated_route)) => {
                debug!("Route refreshed: {}", path);
                updated_route
            }
            Ok(None) => route,
            Err(e) => {
                debug!("Failed to refresh route {}: {}", path, e);
                route
            }
        }
    } else {
        route
    };

    // Extract Accept-Encoding from request headers for our compression system
    let accept_encoding = request_headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    // Use our compression system with pre-computed compressed files
    let (encoding, content) = route.content.get_best_encoding(accept_encoding);

    // Build response with optimized headers
    let mut response_headers = HeaderMap::new();

    response_headers.insert(
        header::CONTENT_TYPE,
        route.headers.content_type.parse().unwrap(),
    );
    response_headers.insert(
        header::LAST_MODIFIED,
        route.headers.last_modified.parse().unwrap(),
    );
    response_headers.insert(header::ETAG, route.headers.etag.parse().unwrap());
    response_headers.insert(
        header::CACHE_CONTROL,
        route.headers.cache_control.parse().unwrap(),
    );

    // Add our compression encoding header if compressed
    if encoding != "identity" {
        response_headers.insert(header::CONTENT_ENCODING, encoding.parse().unwrap());
    }

    debug!(
        "Serving {} bytes with encoding: {} (from pre-compressed cache)",
        content.len(),
        encoding
    );

    (StatusCode::OK, response_headers, content.clone()).into_response()
}
