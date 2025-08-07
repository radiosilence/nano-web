use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nano_web::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

// Benchmark route lookup performance
fn bench_route_lookup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let routes = Arc::new(server::NanoServer::new());

        // Create test routes
        for i in 0..1000 {
            let path = format!("/test-{}.html", i);
            let content = format!("<html>Test content {}</html>", i).into_bytes();
            let route = create_test_route(content);
            routes.insert(path, route).await;
        }

        let mut group = c.benchmark_group("route_lookup");

        for size in [1, 10, 100, 1000].iter() {
            group.throughput(Throughput::Elements(*size as u64));
            group.bench_with_input(BenchmarkId::new("get", size), size, |b, &size| {
                b.to_async(&rt).iter(|| async {
                    for i in 0..size {
                        let path = format!("/test-{}.html", i % 1000);
                        black_box(routes.get(&path).await);
                    }
                });
            });
        }
        group.finish();
    });
}

// Benchmark compression performance
fn bench_compression(c: &mut Criterion) {
    let small_content = "Hello, World!".repeat(100);
    let medium_content = "Hello, World!".repeat(1000);
    let large_content = "Hello, World!".repeat(10000);

    let mut group = c.benchmark_group("compression");

    for (name, content) in [
        ("small", &small_content),
        ("medium", &medium_content),
        ("large", &large_content),
    ] {
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(BenchmarkId::new("gzip", name), content, |b, content| {
            b.iter(|| {
                black_box(compression::gzip_compress(content.as_bytes()).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("brotli", name), content, |b, content| {
            b.iter(|| {
                black_box(compression::brotli_compress(content.as_bytes()).unwrap());
            });
        });

        group.bench_with_input(BenchmarkId::new("zstd", name), content, |b, content| {
            b.iter(|| {
                black_box(compression::zstd_compress(content.as_bytes()).unwrap());
            });
        });
    }
    group.finish();
}

// Benchmark MIME type detection
fn bench_mime_detection(c: &mut Criterion) {
    let paths = vec![
        "/test.html",
        "/test.css",
        "/test.js",
        "/test.png",
        "/test.jpg",
        "/test.svg",
        "/test.woff2",
        "/test.json",
        "/very/long/path/to/some/deeply/nested/file.html",
        "/index.html",
        "/style.css",
        "/app.js",
        "/logo.png",
    ];

    let mut group = c.benchmark_group("mime_detection");
    group.throughput(Throughput::Elements(paths.len() as u64));

    group.bench_function("mime_guess", |b| {
        b.iter(|| {
            for path in &paths {
                black_box(mime_types::get_mime_config(path));
            }
        });
    });

    group.finish();
}

// Benchmark memory allocation patterns
fn bench_memory_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Test Vec vs Bytes for content storage
    let content = "Hello, World!".repeat(1000);

    group.bench_function("vec_clone", |b| {
        b.iter(|| {
            let v: Vec<u8> = content.as_bytes().to_vec();
            black_box(v.clone());
        });
    });

    group.bench_function("bytes_clone", |b| {
        let content_bytes = content.as_bytes().to_vec();
        b.iter(|| {
            let bytes = Bytes::from(content_bytes.clone());
            black_box(bytes.clone());
        });
    });

    // Test different HashMap implementations for routing
    group.bench_function("hashmap_insert", |b| {
        b.iter(|| {
            let mut map = std::collections::HashMap::new();
            for i in 0..100 {
                map.insert(format!("/path-{}", i), i);
            }
            black_box(map);
        });
    });

    group.bench_function("btreemap_insert", |b| {
        b.iter(|| {
            let mut map = std::collections::BTreeMap::new();
            for i in 0..100 {
                map.insert(format!("/path-{}", i), i);
            }
            black_box(map);
        });
    });

    group.finish();
}

// Benchmark template rendering
fn bench_template_rendering(c: &mut Criterion) {
    let simple_template = "<html><title>{{title}}</title><body>{{content}}</body></html>";
    let complex_template = r#"
<!DOCTYPE html>
<html>
<head>
    <title>{{title}}</title>
    <meta charset="utf-8">
    {{#each styles}}
    <link rel="stylesheet" href="{{this}}">
    {{/each}}
</head>
<body>
    <h1>{{header}}</h1>
    {{#each items}}
    <div class="item">
        <h2>{{title}}</h2>
        <p>{{content}}</p>
        {{#if link}}<a href="{{link}}">Read more</a>{{/if}}
    </div>
    {{/each}}
</body>
</html>
"#;

    let mut group = c.benchmark_group("template_rendering");

    group.bench_function("simple", |b| {
        b.iter(|| {
            black_box(template::render_template(simple_template, "VITE_").unwrap_or_default());
        });
    });

    group.bench_function("complex", |b| {
        b.iter(|| {
            black_box(template::render_template(complex_template, "VITE_").unwrap_or_default());
        });
    });

    group.finish();
}

// End-to-end latency benchmark
fn bench_e2e_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let routes = Arc::new(server::NanoServer::new());
        let test_content = "<html><body>Hello World</body></html>".as_bytes();
        let route = create_test_route(test_content.to_vec());
        routes.insert("/".to_string(), route).await;

        let mut group = c.benchmark_group("e2e_latency");
        group.measurement_time(Duration::from_secs(10));
        group.sample_size(10000);

        group.bench_function("full_request_cycle", |b| {
            b.to_async(&rt).iter(|| async {
                // Simulate full request cycle: lookup + headers + compression check
                let route = routes.get("/").await.unwrap();
                let accept_encoding = "gzip, br, zstd";
                let (_encoding, content) = route.content.get_best_encoding(accept_encoding);
                black_box(content);
            });
        });

        group.finish();
    });
}

fn create_test_route(content: Vec<u8>) -> server::FastRoute {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mime_config = mime_types::get_mime_config("test.html");
    let compressed =
        compression::CompressedContent::new(content, mime_config.is_compressible).unwrap();
    let modified = SystemTime::now();

    let etag = format!(
        "\"{}\"",
        modified.duration_since(UNIX_EPOCH).unwrap().as_nanos()
    );
    let last_modified = "Wed, 21 Oct 2015 07:28:00 GMT".to_string();
    let cache_control = mime_types::get_cache_control(&mime_config.mime_type).to_string();

    let headers = server::FastRouteHeaders {
        content_type: mime_config.mime_type,
        last_modified,
        etag,
        cache_control,
    };

    server::FastRoute {
        content: compressed,
        path: PathBuf::from("test.html"),
        modified,
        headers,
    }
}

criterion_group!(
    benches,
    bench_route_lookup,
    bench_compression,
    bench_mime_detection,
    bench_memory_allocations,
    bench_template_rendering,
    bench_e2e_latency
);
criterion_main!(benches);
