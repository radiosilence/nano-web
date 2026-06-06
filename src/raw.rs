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

use crate::routes::NanoWeb;
use crate::server::{create_reuse_port_listener, ServeConfig};
use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

const NOT_FOUND: &[u8] =
    b"HTTP/1.1 404 Not Found\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 9\r\n\r\nNot Found";
const BAD_REQUEST: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 11\r\n\r\nBad Request";
const METHOD_NOT_ALLOWED: &[u8] =
    b"HTTP/1.1 405 Method Not Allowed\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 18\r\n\r\nMethod Not Allowed";

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
    let method = req.method.unwrap_or("");
    let is_head = method == "HEAD";
    if method != "GET" && !is_head {
        stream.write_all(METHOD_NOT_ALLOWED).await?;
        return Ok(());
    }

    // httparse path includes the query string; strip it like hyper's uri().path().
    let raw_path = req.path.unwrap_or("/");
    let raw_path = raw_path.split('?').next().unwrap_or("/");

    if raw_path == "/_health" {
        let body = br#"{"status":"ok"}"#;
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(body).await?;
        return Ok(());
    }

    let Ok(path) = crate::path::validate_request_path(raw_path) else {
        stream.write_all(BAD_REQUEST).await?;
        return Ok(());
    };

    let accept_encoding = header(req, "accept-encoding").unwrap_or("");
    let mut rb = server.get_response(&path, accept_encoding);
    if rb.is_none() && !path.ends_with('/') {
        rb = server.get_response(&format!("{path}/"), accept_encoding);
    }
    if rb.is_none() && spa {
        rb = server.get_response("/", accept_encoding);
    }

    match rb {
        Some(b) => {
            // ETag conditional → 304 with the minimal validator header set.
            if let Some(inm) = header(req, "if-none-match") {
                if inm == b.etag.as_ref() {
                    let head = format!(
                        "HTTP/1.1 304 Not Modified\r\netag: {}\r\ncache-control: {}\r\n\r\n",
                        b.etag, b.cache_control
                    );
                    stream.write_all(head.as_bytes()).await?;
                    return Ok(());
                }
            }
            stream.write_all(&b.head).await?;
            if !is_head {
                stream.write_all(&b.body).await?;
            }
        }
        None => stream.write_all(NOT_FOUND).await?,
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
