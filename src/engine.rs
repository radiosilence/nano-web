//! The transport-agnostic core every fast engine shares.
//!
//! A request handler is just: parse bytes into [`Request`], call [`route`], write
//! the resulting [`Reply`]. `route` makes the entire HTTP decision — method, path
//! sanitation, encoding, conditional, SPA fallback — and returns *precomputed
//! bytes to write*. It allocates nothing on the hit path and knows nothing about
//! sockets, runtimes, or `io_uring`. Each engine (tokio/kqueue, `io_uring`, …)
//! only supplies the read and the write in its platform's dialect.

use crate::response_buffer::ResponseBuffer;
use crate::routes::NanoWeb;
use std::sync::Arc;

// Fully static responses — no per-request formatting. Content-Length is baked in.
pub const NOT_FOUND: &[u8] =
    b"HTTP/1.1 404 Not Found\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 9\r\n\r\nNot Found";
pub const BAD_REQUEST: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 11\r\n\r\nBad Request";
pub const METHOD_NOT_ALLOWED: &[u8] =
    b"HTTP/1.1 405 Method Not Allowed\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 18\r\n\r\nMethod Not Allowed";
pub const HEALTH: &[u8] =
    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 15\r\n\r\n{\"status\":\"ok\"}";

/// A request reduced to only what routing needs. All fields borrow the engine's
/// read buffer — no copies.
pub struct Request<'a> {
    pub method: &'a str,
    /// Raw request target, query string included (`route` strips it).
    pub path: &'a str,
    pub accept_encoding: &'a str,
    pub if_none_match: Option<&'a str>,
}

/// What to put on the wire. Every variant is precomputed bytes — the engine just
/// writes them. No allocation, no formatting.
pub enum Reply {
    /// 200: write `buf.head`, then `buf.body` unless `body` is false (HEAD).
    Ok {
        buf: Arc<ResponseBuffer>,
        body: bool,
    },
    /// 304: write `buf.head_304` (validators only).
    NotModified { buf: Arc<ResponseBuffer> },
    /// Fixed response (404/400/405/health) — fully static bytes.
    Static(&'static [u8]),
}

/// The shared routing decision. Allocation-free on the hit path; the only possible
/// allocation is the trailing-slash retry key on a miss.
pub fn route(server: &NanoWeb, req: &Request, spa: bool) -> Reply {
    let is_head = req.method == "HEAD";
    if req.method != "GET" && !is_head {
        return Reply::Static(METHOD_NOT_ALLOWED);
    }

    let path = req.path.split('?').next().unwrap_or("/");
    if path == "/_health" {
        return Reply::Static(HEALTH);
    }

    let Ok(path) = crate::path::validate_request_path(path) else {
        return Reply::Static(BAD_REQUEST);
    };

    let mut rb = server.get_response(&path, req.accept_encoding);
    if rb.is_none() && !path.ends_with('/') {
        rb = server.get_response(&format!("{path}/"), req.accept_encoding);
    }
    if rb.is_none() && spa {
        rb = server.get_response("/", req.accept_encoding);
    }

    match rb {
        Some(buf) => {
            if let Some(inm) = req.if_none_match {
                if inm == buf.etag.as_ref() {
                    return Reply::NotModified { buf };
                }
            }
            Reply::Ok {
                buf,
                body: !is_head,
            }
        }
        None => Reply::Static(NOT_FOUND),
    }
}
