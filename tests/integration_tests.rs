use reqwest::StatusCode;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};

/// Bind to port 0 and let the OS assign a free port, avoiding collisions in parallel test runs.
fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

// Helper to create a test server
fn create_test_server(
    temp_dir: &Path,
    port: u16,
    spa_mode: bool,
    dev_mode: bool,
) -> tokio::task::JoinHandle<()> {
    create_test_server_engine(
        temp_dir,
        port,
        spa_mode,
        dev_mode,
        nano_web::server::Engine::Hyper,
    )
}

fn create_test_server_engine(
    temp_dir: &Path,
    port: u16,
    spa_mode: bool,
    dev_mode: bool,
    engine: nano_web::server::Engine,
) -> tokio::task::JoinHandle<()> {
    let config = nano_web::server::ServeConfig {
        public_dir: temp_dir.to_path_buf(),
        port,
        dev: dev_mode,
        spa_mode,
        config_prefix: "TEST_".to_string(),
        log_requests: false,
        engine,
    };

    tokio::spawn(async move {
        nano_web::server::start_server(config).await.unwrap();
    })
}

#[tokio::test]
async fn test_spa_mode_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create index.html
    fs::write(
        temp_path.join("index.html"),
        r#"<html><body><div id="app">SPA App</div></body></html>"#,
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, true, false);
    sleep(Duration::from_millis(100)).await;

    // Test that existing routes work
    let response = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SPA App"));

    // Test that non-existent routes fallback to index.html (SPA behavior)
    let response = reqwest::get(format!("http://localhost:{port}/nonexistent/route"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SPA App")); // Should serve index.html
}

#[tokio::test]
async fn test_non_spa_mode_404() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create index.html
    fs::write(
        temp_path.join("index.html"),
        "<html><body>Regular App</body></html>",
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // Test that existing routes work
    let response = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test that non-existent routes return 404
    let response = reqwest::get(format!("http://localhost:{port}/nonexistent"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_dev_mode_file_reloading() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create initial file
    let test_file = temp_path.join("test.html");
    fs::write(&test_file, "<html><body>Version 1</body></html>").unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, true);
    sleep(Duration::from_millis(100)).await;

    // Test initial content
    let response = reqwest::get(format!("http://localhost:{port}/test.html"))
        .await
        .unwrap();
    let body = response.text().await.unwrap();
    assert!(body.contains("Version 1"));

    // Update file
    sleep(Duration::from_millis(50)).await; // Ensure different timestamp
    fs::write(&test_file, "<html><body>Version 2</body></html>").unwrap();

    // Test updated content (dev mode should reload)
    sleep(Duration::from_millis(50)).await;
    let response = reqwest::get(format!("http://localhost:{port}/test.html"))
        .await
        .unwrap();
    let body = response.text().await.unwrap();
    assert!(body.contains("Version 2"));
}

#[tokio::test]
#[allow(unsafe_code)]
async fn test_template_rendering() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // SAFETY: single-threaded test context, cleaned up below
    unsafe { std::env::set_var("TEST_API_URL", "http://test.api.com") };

    // Create HTML file with template
    let template_content = r#"
    <html>
    <head>
        <script>
            window.ENV = JSON.parse("{{EscapedJson}}");
            window.API_URL = "{{env.API_URL}}";
        </script>
    </head>
    <body>Config injected</body>
    </html>
    "#;

    fs::write(temp_path.join("index.html"), template_content).unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // Test that template is rendered
    let response = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    let body = response.text().await.unwrap();

    assert!(body.contains("http://test.api.com"));
    assert!(body.contains("window.ENV = JSON.parse"));
    assert!(!body.contains("{{EscapedJson}}")); // Template should be processed

    // SAFETY: single-threaded cleanup
    unsafe { std::env::remove_var("TEST_API_URL") };
}

#[tokio::test]
async fn test_health_endpoint() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let response = reqwest::get(format!("http://localhost:{port}/_health"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.unwrap();
    assert!(body.contains(r#""status":"ok""#));
}

#[tokio::test]
async fn test_compression_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a larger file that should be compressed (needs to be >= 1024 bytes)
    let large_content = "x".repeat(2000);
    fs::write(temp_path.join("large.txt"), &large_content).unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // Request with Accept-Encoding
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:{port}/large.txt"))
        .header("Accept-Encoding", "gzip, br")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have compression headers
    let headers = response.headers();
    assert!(
        headers.contains_key("content-encoding"),
        "Expected content-encoding header for compressed response"
    );
}

#[tokio::test]
async fn test_security_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(
        temp_path.join("test.html"),
        "<html><body>Test</body></html>",
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let response = reqwest::get(format!("http://localhost:{port}/test.html"))
        .await
        .unwrap();
    let headers = response.headers();

    // Check security headers
    assert!(headers.contains_key("x-content-type-options"));
    assert!(headers.contains_key("x-frame-options"));
    assert_eq!(headers.get("x-frame-options").unwrap(), "SAMEORIGIN");
}

#[tokio::test]
async fn test_path_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(temp_path.join("safe.txt"), "safe content").unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // Test path traversal attempts - hidden files should be blocked (except .well-known)
    let hidden_file_paths = ["/.env", "/.secret"];

    for path in hidden_file_paths {
        let url = format!("http://localhost:{port}{path}");
        let response = reqwest::get(&url).await.unwrap();

        // Should return 400 Bad Request for hidden files
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "Path: {path}");
    }

    // Test that normal path traversal (which gets normalized by HTTP stack) returns 404
    let normalized_paths = ["/../../../etc/passwd"];
    for path in normalized_paths {
        let url = format!("http://localhost:{port}{path}");
        let response = reqwest::get(&url).await.unwrap();

        // These get normalized by HTTP stack and just return 404 (not found)
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "Path: {path}");
    }

    // Test that .well-known paths are allowed (but return 404 if file doesn't exist)
    let wellknown_paths = [
        "/.well-known/security.txt",
        "/.well-known/acme-challenge/token",
    ];
    for path in wellknown_paths {
        let url = format!("http://localhost:{port}{path}");
        let response = reqwest::get(&url).await.unwrap();

        // Should return 404 (not found) not 400 (bad request) - meaning path validation passed
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "Path: {path}");
    }

    // But safe paths should work
    let response = reqwest::get(format!("http://localhost:{port}/safe.txt"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_non_compressible_with_accept_encoding() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a fake PNG (non-compressible file type)
    let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    fs::write(temp_path.join("image.png"), png_header).unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // Request with Accept-Encoding header (like browsers do)
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:{port}/image.png"))
        .header("Accept-Encoding", "gzip, deflate, br, zstd")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Non-compressible file should return 200 even with Accept-Encoding"
    );
    assert_eq!(response.headers().get("content-type").unwrap(), "image/png");
}

#[tokio::test]
async fn test_head_returns_empty_body() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(
        temp_path.join("test.html"),
        "<html><body>Hello</body></html>",
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client
        .head(format!("http://localhost:{port}/test.html"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("content-type"));
    assert!(response.headers().contains_key("etag"));

    // HEAD should return empty body
    let body = response.text().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn test_etag_304_not_modified() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(
        temp_path.join("test.html"),
        "<html><body>Cached</body></html>",
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // First request to get the ETag
    let response = reqwest::get(format!("http://localhost:{port}/test.html"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Second request with If-None-Match should return 304
    let client = reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .unwrap();
    let response = client
        .get(format!("http://localhost:{port}/test.html"))
        .header("If-None-Match", &etag)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn test_method_not_allowed() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(temp_path.join("test.html"), "<html></html>").unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://localhost:{port}/test.html"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_new_security_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    fs::write(
        temp_path.join("test.html"),
        "<html><body>Test</body></html>",
    )
    .unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let response = reqwest::get(format!("http://localhost:{port}/test.html"))
        .await
        .unwrap();
    let headers = response.headers();

    assert_eq!(
        headers.get("strict-transport-security").unwrap(),
        "max-age=63072000; includeSubDomains"
    );
    assert_eq!(
        headers.get("permissions-policy").unwrap(),
        "camera=(), microphone=(), geolocation=()"
    );
    assert_eq!(headers.get("x-dns-prefetch-control").unwrap(), "off");
}

#[tokio::test]
async fn test_vary_header_on_compressed() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let large_content = "x".repeat(2000);
    fs::write(temp_path.join("large.txt"), &large_content).unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:{port}/large.txt"))
        .header("Accept-Encoding", "gzip, br")
        .send()
        .await
        .unwrap();

    assert_eq!(response.headers().get("vary").unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn test_content_length_header() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let content = "Hello, World!";
    fs::write(temp_path.join("hello.txt"), content).unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .unwrap();
    let response = client
        .get(format!("http://localhost:{port}/hello.txt"))
        .send()
        .await
        .unwrap();

    assert!(response.headers().contains_key("content-length"));
}

#[tokio::test]
async fn test_cache_control_values() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    fs::write(temp_path.join("page.html"), "<html></html>").unwrap();
    fs::write(temp_path.join("style.css"), "body{}").unwrap();
    fs::write(temp_path.join("data.json"), "{}").unwrap();

    let port = get_free_port();
    let _server = create_test_server(temp_path, port, false, false);
    sleep(Duration::from_millis(100)).await;

    // HTML: 15 minutes
    let resp = reqwest::get(format!("http://localhost:{port}/page.html"))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=900"
    );

    // CSS: 1 year immutable (asset)
    let resp = reqwest::get(format!("http://localhost:{port}/style.css"))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );

    // JSON: 1 hour (other)
    let resp = reqwest::get(format!("http://localhost:{port}/data.json"))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=3600"
    );
}

// ── Raw engine ───────────────────────────────────────────────────────────────
// The raw engine (hand-rolled HTTP/1.1, no hyper) must match the production
// engine's observable behaviour. These mirror the core hyper-path assertions.

use nano_web::server::Engine;

#[tokio::test]
async fn test_raw_serves_body_and_headers() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("index.html"), "<h1>raw</h1>").unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/html");
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(resp.headers().get("etag").is_some());
    assert_eq!(resp.headers().get("content-length").unwrap(), "12");
    assert_eq!(resp.text().await.unwrap(), "<h1>raw</h1>");
}

#[tokio::test]
async fn test_raw_brotli_encoding() {
    let temp_dir = TempDir::new().unwrap();
    // >1KB compressible so compression kicks in
    fs::write(temp_dir.path().join("app.js"), "// comment\n".repeat(200)).unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://localhost:{port}/app.js"))
        .header("accept-encoding", "br")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-encoding").unwrap(), "br");
    assert_eq!(resp.headers().get("vary").unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn test_raw_404_and_405() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("index.html"), "<h1>raw</h1>").unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    let resp = reqwest::get(format!("http://localhost:{port}/missing"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = reqwest::Client::new()
        .post(format!("http://localhost:{port}/"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_raw_head_and_304() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("index.html"), "<h1>raw</h1>").unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    // HEAD: headers present, body empty
    let resp = reqwest::Client::new()
        .head(format!("http://localhost:{port}/"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-length").unwrap(), "12");
    assert!(resp.bytes().await.unwrap().is_empty());

    // Fetch etag, then conditional GET → 304
    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    let etag = resp
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let resp = reqwest::Client::new()
        .get(format!("http://localhost:{port}/"))
        .header("if-none-match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn test_raw_health() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("index.html"), "<h1>raw</h1>").unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    let resp = reqwest::get(format!("http://localhost:{port}/_health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.text().await.unwrap().contains("ok"));
}

#[tokio::test]
async fn test_raw_path_traversal_blocked() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("index.html"), "<h1>raw</h1>").unwrap();

    let port = get_free_port();
    let _server = create_test_server_engine(temp_dir.path(), port, false, false, Engine::Raw);
    sleep(Duration::from_millis(100)).await;

    // reqwest normalizes ../ client-side, so hit the socket raw.
    let mut s = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    s.write_all(b"GET /../../etc/passwd HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).await.unwrap();
    assert!(
        resp.starts_with("HTTP/1.1 400"),
        "expected 400, got: {}",
        &resp[..resp.len().min(40)]
    );
}
