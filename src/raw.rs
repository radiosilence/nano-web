//! Experimental hand-rolled HTTP/1.1 engine.
//!
//! No HTTP library in the hot path: parse just enough of the request with
//! `httparse`, look up the precomputed `ResponseBuffer`, and write its
//! pre-serialized `head` (and `body` for GET) straight to the socket. This is
//! the userspace half of the "precomputed bytes → socket" idea — the part that
//! runs without `io_uring`, so it can be measured on any platform.
//!
//! Deliberately omits: dev-mode reload, request logging, and a Date header.
//! It exists to measure the ceiling of the no-library path, not to replace the
//! production engine.

use crate::engine::{self, Reply};
use crate::routes::NanoWeb;
use crate::server::{create_reuse_port_listener, ServeConfig};
use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

pub async fn start_server(config: ServeConfig) -> Result<()> {
    let server = Arc::new(NanoWeb::new());
    server.populate_routes(&config.public_dir, &config.config_prefix)?;
    info!("Routes loaded: {} (raw engine)", server.route_count());

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let listener = TcpListener::from_std(create_reuse_port_listener(addr)?)?;
    info!("Starting server on http://{} (raw engine)", addr);

    let spa = config.spa_mode;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let server = server.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, &server, spa).await {
                        debug!("connection error: {e}");
                    }
                });
            }
            () = shutdown() => {
                info!("Shutdown signal received, stopping server");
                return Ok(());
            }
        }
    }
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Serve requests on a single keep-alive connection until the client closes,
/// sends `Connection: close`, or errors.
async fn handle_conn(mut stream: TcpStream, server: &NanoWeb, spa: bool) -> Result<()> {
    let _ = stream.set_nodelay(true);
    let mut buf = vec![0u8; 8192];
    let mut len = 0usize;

    loop {
        // Accumulate bytes until a full request head parses.
        let (consumed, keep_alive) = loop {
            let mut headers = [httparse::EMPTY_HEADER; 32];
            let mut req = httparse::Request::new(&mut headers);
            let mut done: Option<(usize, bool)> = None;

            match req.parse(&buf[..len])? {
                httparse::Status::Complete(n) => {
                    let keep_alive = !connection_close(&req);
                    respond(&mut stream, server, &req, spa).await?;
                    done = Some((n, keep_alive));
                }
                httparse::Status::Partial => {}
            }
            // `req`/`headers` borrow `buf`; they drop here, before we touch buf.
            if let Some(d) = done {
                break d;
            }

            if len == buf.len() {
                buf.resize(len * 2, 0);
            }
            let n = stream.read(&mut buf[len..]).await?;
            if n == 0 {
                return Ok(()); // client closed (cleanly if mid-idle)
            }
            len += n;
        };

        // Drop the consumed request, keep any pipelined leftover for the next loop.
        buf.copy_within(consumed..len, 0);
        len -= consumed;

        if !keep_alive {
            return Ok(());
        }
    }
}

async fn respond(
    stream: &mut TcpStream,
    server: &NanoWeb,
    req: &httparse::Request<'_, '_>,
    spa: bool,
) -> Result<()> {
    // Extract the four fields routing needs, then let the shared core decide.
    let parsed = engine::Request {
        method: req.method.unwrap_or(""),
        path: req.path.unwrap_or("/"),
        accept_encoding: header(req, "accept-encoding").unwrap_or(""),
        if_none_match: header(req, "if-none-match"),
    };

    match engine::route(server, &parsed, spa) {
        // One writev for head+body — two write_all calls would be two syscalls
        // (and two segments under TCP_NODELAY), which cost ~18% on bodied GETs.
        Reply::Ok { buf, body: true } => write_head_body(stream, &buf.head, &buf.body).await?,
        Reply::Ok { buf, body: false } => stream.write_all(&buf.head).await?,
        Reply::NotModified { buf } => stream.write_all(&buf.head_304).await?,
        Reply::Static(bytes) => stream.write_all(bytes).await?,
    }
    Ok(())
}

/// Write `head` then `body` in a single `writev` where the OS allows it, looping
/// only to drain partial writes (large bodies). `TcpStream` reports vectored
/// support, so small responses leave in one syscall with no copy and no extra
/// allocation.
async fn write_head_body(stream: &mut TcpStream, head: &[u8], body: &[u8]) -> std::io::Result<()> {
    use std::io::IoSlice;
    let total = head.len() + body.len();
    let mut off = 0usize;
    while off < total {
        let (h, b) = if off < head.len() {
            (&head[off..], body)
        } else {
            (&[][..], &body[off - head.len()..])
        };
        let n = stream
            .write_vectored(&[IoSlice::new(h), IoSlice::new(b)])
            .await?;
        if n == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        off += n;
    }
    Ok(())
}

/// Case-insensitive header lookup against the parsed request.
fn header<'a>(req: &'a httparse::Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .and_then(|h| std::str::from_utf8(h.value).ok())
}

/// HTTP/1.1 keeps connections alive unless the client says otherwise.
fn connection_close(req: &httparse::Request) -> bool {
    header(req, "connection").is_some_and(|v| v.eq_ignore_ascii_case("close"))
}
