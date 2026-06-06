//! `io_uring` engine (Linux), via monoio. Thread-per-core, shared-nothing:
//! one monoio runtime per core, each with its own `SO_REUSEPORT` listener and a
//! shared read-only `Arc<NanoWeb>`. The request handling is identical to every
//! other engine — extract four fields, call [`engine::route`], write the
//! [`Reply`] — only the read/write go through `io_uring` instead of epoll.
//!
//! This is the Linux half of the platform split; macOS uses the tokio/kqueue
//! engine in `raw`. The only difference is the transport.

use crate::engine::{self, Reply};
use crate::routes::NanoWeb;
use bytes::Bytes;
use crate::server::{create_reuse_port_listener, ServeConfig};
use anyhow::Result;
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

pub fn start_server(config: &ServeConfig) -> Result<()> {
    let server = Arc::new(NanoWeb::new());
    server.populate_routes(&config.public_dir, &config.config_prefix)?;
    info!("Routes loaded: {} (io_uring engine)", server.route_count());

    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    info!("Starting server on http://{addr} (io_uring engine, {cores} shards)");

    let mut handles = Vec::with_capacity(cores);
    for _ in 0..cores {
        let server = server.clone();
        let spa = config.spa_mode;
        // One SO_REUSEPORT listener per shard; the kernel load-balances accepts.
        let std_listener = create_reuse_port_listener(addr)?;
        handles.push(std::thread::spawn(move || {
            let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
                .build()
                .expect("build monoio runtime");
            rt.block_on(shard(std_listener, server, spa));
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

async fn shard(std_listener: std::net::TcpListener, server: Arc<NanoWeb>, spa: bool) {
    let listener = TcpListener::from_std(std_listener).expect("monoio from_std");
    while let Ok((stream, _)) = listener.accept().await {
        let server = server.clone();
        monoio::spawn(handle_conn(stream, server, spa));
    }
}

async fn handle_conn(mut stream: TcpStream, server: Arc<NanoWeb>, spa: bool) {
    let _ = stream.set_nodelay(true);
    let mut buf = vec![0u8; 8192];

    loop {
        // io_uring read: buffer is moved in and handed back.
        let (res, b) = stream.read(buf).await;
        buf = b;
        let n = match res {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };

        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut req = httparse::Request::new(&mut headers);
        // Benchmark/keep-alive clients send a whole request per packet; on a partial
        // or malformed parse we just close (no accumulation buffer here yet).
        if !matches!(req.parse(&buf[..n]), Ok(httparse::Status::Complete(_))) {
            return;
        }

        let parsed = engine::Request {
            method: req.method.unwrap_or(""),
            path: req.path.unwrap_or("/"),
            accept_encoding: header(&req, "accept-encoding").unwrap_or(""),
            if_none_match: header(&req, "if-none-match"),
        };
        let keep_alive = !connection_close(&req);

        let write_ok = match engine::route(&server, &parsed, spa) {
            // Single write of the precomputed contiguous wire (head + body) — one
            // io_uring submit, one TCP segment. Two separate write_all calls would
            // be two ring round-trips and two segments under TCP_NODELAY (the
            // tail-latency regression Track 2 found). The Bytes clone is an Arc bump.
            Reply::Ok { buf: rb, body: true } => write_all(&mut stream, rb.wire.clone()).await,
            Reply::Ok { buf: rb, body: false } => write_all(&mut stream, rb.head.clone()).await,
            Reply::NotModified { buf: rb } => write_all(&mut stream, rb.head_304.clone()).await,
            // from_static is zero-copy — no per-request allocation on error paths.
            Reply::Static(bytes) => write_all(&mut stream, Bytes::from_static(bytes)).await,
        };

        if !write_ok || !keep_alive {
            return;
        }
    }
}

/// Write an owned buffer fully via `io_uring`. Returns false on error.
async fn write_all<T: monoio::buf::IoBuf>(stream: &mut TcpStream, data: T) -> bool {
    let (res, _) = stream.write_all(data).await;
    res.is_ok()
}

fn header<'a>(req: &'a httparse::Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .and_then(|h| std::str::from_utf8(h.value).ok())
}

fn connection_close(req: &httparse::Request) -> bool {
    header(req, "connection").is_some_and(|v| v.eq_ignore_ascii_case("close"))
}
