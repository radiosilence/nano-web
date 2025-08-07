use reqwest::StatusCode;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

// Helper to create a test server
async fn create_test_server(
    temp_dir: &Path,
    port: u16,
    spa_mode: bool,
    dev_mode: bool,
) -> tokio::task::JoinHandle<()> {
    let config = nano_web::axum_server::AxumServeConfig {
        public_dir: temp_dir.to_path_buf(),
        port,
        dev: dev_mode,
        spa_mode,
        config_prefix: "TEST_".to_string(),
        log_requests: false,
    };

    tokio::spawn(async move {
        nano_web::axum_server::start_axum_server(config).await.unwrap();
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

    // Start server with SPA mode
    let _server = create_test_server(temp_path, 3001, true, false).await;
    sleep(Duration::from_millis(100)).await;

    // Test that existing routes work
    let response = reqwest::get("http://localhost:3001/").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SPA App"));

    // Test that non-existent routes fallback to index.html (SPA behavior)
    let response = reqwest::get("http://localhost:3001/nonexistent/route").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("SPA App")); // Should serve index.html
}

#[tokio::test]
async fn test_non_spa_mode_404() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create index.html
    fs::write(temp_path.join("index.html"), "<html><body>Regular App</body></html>").unwrap();

    // Start server without SPA mode
    let _server = create_test_server(temp_path, 3002, false, false).await;
    sleep(Duration::from_millis(100)).await;

    // Test that existing routes work
    let response = reqwest::get("http://localhost:3002/").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test that non-existent routes return 404
    let response = reqwest::get("http://localhost:3002/nonexistent").await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_dev_mode_file_reloading() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create initial file
    let test_file = temp_path.join("test.html");
    fs::write(&test_file, "<html><body>Version 1</body></html>").unwrap();

    // Start server with dev mode
    let _server = create_test_server(temp_path, 3003, false, true).await;
    sleep(Duration::from_millis(100)).await;

    // Test initial content
    let response = reqwest::get("http://localhost:3003/test.html").await.unwrap();
    let body = response.text().await.unwrap();
    assert!(body.contains("Version 1"));

    // Update file
    sleep(Duration::from_millis(50)).await; // Ensure different timestamp
    fs::write(&test_file, "<html><body>Version 2</body></html>").unwrap();

    // Test updated content (dev mode should reload)
    sleep(Duration::from_millis(50)).await;
    let response = reqwest::get("http://localhost:3003/test.html").await.unwrap();
    let body = response.text().await.unwrap();
    assert!(body.contains("Version 2"));
}

#[tokio::test]
async fn test_template_rendering() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Set test environment variable
    std::env::set_var("TEST_API_URL", "http://test.api.com");

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

    // Start server
    let _server = create_test_server(temp_path, 3004, false, false).await;
    sleep(Duration::from_millis(100)).await;

    // Test that template is rendered
    let response = reqwest::get("http://localhost:3004/").await.unwrap();
    let body = response.text().await.unwrap();

    assert!(body.contains("http://test.api.com"));
    assert!(body.contains("window.ENV = JSON.parse"));
    assert!(!body.contains("{{EscapedJson}}")); // Template should be processed

    // Clean up
    std::env::remove_var("TEST_API_URL");
}

#[tokio::test]
async fn test_health_endpoint() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let _server = create_test_server(temp_path, 3005, false, false).await;
    sleep(Duration::from_millis(100)).await;

    let response = reqwest::get("http://localhost:3005/_health").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.unwrap();
    assert!(body.contains(r#""status":"ok""#));
}

#[tokio::test]
async fn test_compression_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a larger file that should be compressed
    let large_content = "x".repeat(1000);
    fs::write(temp_path.join("large.txt"), &large_content).unwrap();

    let _server = create_test_server(temp_path, 3006, false, false).await;
    sleep(Duration::from_millis(100)).await;

    // Request with Accept-Encoding
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:3006/large.txt")
        .header("Accept-Encoding", "gzip, br")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have compression headers
    let headers = response.headers();
    assert!(
        headers.contains_key("content-encoding")
            || headers.get("content-length").unwrap().to_str().unwrap() != "1000"
    );
}

#[tokio::test]
async fn test_security_headers() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(temp_path.join("test.html"), "<html><body>Test</body></html>").unwrap();

    let _server = create_test_server(temp_path, 3007, false, false).await;
    sleep(Duration::from_millis(100)).await;

    let response = reqwest::get("http://localhost:3007/test.html").await.unwrap();
    let headers = response.headers();

    // Check security headers
    assert!(headers.contains_key("x-content-type-options"));
    assert!(headers.contains_key("x-frame-options"));
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
}

#[tokio::test]
async fn test_path_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    fs::write(temp_path.join("safe.txt"), "safe content").unwrap();

    let _server = create_test_server(temp_path, 3008, false, false).await;
    sleep(Duration::from_millis(100)).await;

    // Test various path traversal attempts
    let dangerous_paths = [
        "/../../../etc/passwd",
        "/../../secret.txt", 
        "/.env",
        "/test/../../../etc/passwd",
    ];

    for path in dangerous_paths {
        let url = format!("http://localhost:3008{}", path);
        let response = reqwest::get(&url).await.unwrap();
        
        // Should return 400 Bad Request for dangerous paths
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "Path: {}", path);
    }

    // But safe paths should work
    let response = reqwest::get("http://localhost:3008/safe.txt").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}