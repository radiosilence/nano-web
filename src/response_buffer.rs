use bytes::Bytes;
use hyper::header::{self, HeaderMap, HeaderName, HeaderValue};
use std::sync::Arc;

/// Security headers identical on every 200 response. Stamped from `&'static str`
/// so each insertion is a cheap shared `HeaderValue`, not a parse.
const STATIC_SECURITY_HEADERS: [(HeaderName, &str); 6] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    (
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=63072000; includeSubDomains",
    ),
    (
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=()",
    ),
    (HeaderName::from_static("x-dns-prefetch-control"), "off"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Identity,
    Gzip,
    Brotli,
    Zstd,
}

impl Encoding {
    pub const ALL: [Self; 4] = [Self::Identity, Self::Gzip, Self::Brotli, Self::Zstd];

    /// Parse Accept-Encoding header, priority: br > zstd > gzip > identity.
    /// Splits on comma to avoid substring false positives (e.g. "br" matching "vibrant").
    /// Respects q=0 (encoding explicitly rejected by client).
    pub fn from_accept_encoding(accept: &str) -> Self {
        let mut best = Self::Identity;
        for part in accept.split(',') {
            let mut segments = part.split(';');
            let token = segments.next().unwrap_or("").trim();

            // q=0 means the encoding is explicitly rejected
            let rejected = segments.any(|s| {
                s.trim()
                    .strip_prefix("q=")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .is_some_and(|q| q == 0.0)
            });
            if rejected {
                continue;
            }

            match token {
                "br" => return Self::Brotli, // highest priority, short-circuit
                "zstd" => best = Self::Zstd,
                "gzip" if !matches!(best, Self::Zstd) => best = Self::Gzip,
                _ => {}
            }
        }
        best
    }
}

#[derive(Debug, Clone)]
pub struct ResponseBuffer {
    pub body: Bytes,
    pub content_type: Arc<str>,
    pub content_encoding: Option<&'static str>,
    pub etag: Arc<str>,
    pub last_modified: Arc<str>,
    pub cache_control: Arc<str>,
    pub content_length: Arc<str>,
    /// Whether Vary: Accept-Encoding should be sent (true for all compressible types)
    pub vary_encoding: bool,
    /// Fully-built header block for the 200 response. Precomputed once at route
    /// creation so the hot path clones it instead of re-inserting ~13 headers
    /// per request. The body is appended by the server (or dropped for HEAD).
    pub headers: HeaderMap,
    /// Pre-serialized HTTP/1.1 response head ("HTTP/1.1 200 OK\r\n" + headers +
    /// "\r\n"), ready to write to a socket verbatim. Used by the raw engine,
    /// which writes `head` then `body` with no per-request formatting at all.
    pub head: Bytes,
    /// Pre-serialized `304 Not Modified` response (validators only), so the
    /// conditional path is a single verbatim write — no per-request `format!`.
    pub head_304: Bytes,
    /// Full 200 response (head + body) as one contiguous buffer, so the `io_uring`
    /// engine serves a bodied GET in a single syscall/SQE and a single segment —
    /// monoio can't cheaply do a zero-copy vectored write of two `Bytes`, so we
    /// trade memory (a second copy of the body) for one write. Built only in
    /// uring builds; the tokio engine uses `writev` over `head`+`body` instead.
    #[cfg(all(target_os = "linux", feature = "uring"))]
    pub wire: Bytes,
}

impl ResponseBuffer {
    pub fn new(
        body: Bytes,
        content_type: Arc<str>,
        content_encoding: Option<&'static str>,
        etag: Arc<str>,
        last_modified: Arc<str>,
        cache_control: Arc<str>,
        vary_encoding: bool,
    ) -> Self {
        let content_length: Arc<str> = Arc::from(body.len().to_string().as_str());
        let headers = build_headers(
            &content_type,
            content_encoding,
            &etag,
            &last_modified,
            &cache_control,
            &content_length,
            vary_encoding,
        );
        let head = serialize_head(&headers);
        let head_304 = serialize_304(&etag, &cache_control);
        #[cfg(all(target_os = "linux", feature = "uring"))]
        let wire = {
            let mut v = Vec::with_capacity(head.len() + body.len());
            v.extend_from_slice(&head);
            v.extend_from_slice(&body);
            Bytes::from(v)
        };
        Self {
            body,
            content_type,
            content_encoding,
            etag,
            last_modified,
            cache_control,
            content_length,
            vary_encoding,
            headers,
            head,
            head_304,
            #[cfg(all(target_os = "linux", feature = "uring"))]
            wire,
        }
    }
}

/// Pre-serialize the `304 Not Modified` response (validators only).
fn serialize_304(etag: &str, cache_control: &str) -> Bytes {
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(b"HTTP/1.1 304 Not Modified\r\netag: ");
    buf.extend_from_slice(etag.as_bytes());
    buf.extend_from_slice(b"\r\ncache-control: ");
    buf.extend_from_slice(cache_control.as_bytes());
    buf.extend_from_slice(b"\r\n\r\n");
    Bytes::from(buf)
}

/// Serialize the 200-response status line + header block to raw bytes, ready to
/// write to a socket. Date is intentionally omitted — the raw engine serves
/// immutable precomputed bytes and cannot stamp a live date without per-request
/// work, which is the whole point of avoiding.
fn serialize_head(headers: &HeaderMap) -> Bytes {
    let mut buf = Vec::with_capacity(320);
    buf.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
    for (name, value) in headers {
        buf.extend_from_slice(name.as_str().as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(value.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf.extend_from_slice(b"\r\n");
    Bytes::from(buf)
}

/// Build the complete 200-response header block. All values are server-controlled
/// (mime types, hex etags, HTTP dates, digit content-lengths), so they are always
/// valid header values — an invalid one is a bug, hence `expect`.
fn build_headers(
    content_type: &str,
    content_encoding: Option<&'static str>,
    etag: &str,
    last_modified: &str,
    cache_control: &str,
    content_length: &str,
    vary_encoding: bool,
) -> HeaderMap {
    let mut h = HeaderMap::with_capacity(13);
    let val = |s: &str| HeaderValue::from_str(s).expect("server-controlled header value");

    h.insert(header::CONTENT_TYPE, val(content_type));
    h.insert(header::ETAG, val(etag));
    h.insert(header::LAST_MODIFIED, val(last_modified));
    h.insert(header::CACHE_CONTROL, val(cache_control));
    h.insert(header::CONTENT_LENGTH, val(content_length));
    if let Some(encoding) = content_encoding {
        h.insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
    }
    if vary_encoding {
        h.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    for (name, value) in STATIC_SECURITY_HEADERS {
        h.insert(name, HeaderValue::from_static(value));
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_priority() {
        assert_eq!(
            Encoding::from_accept_encoding("gzip, br, zstd"),
            Encoding::Brotli
        );
        assert_eq!(Encoding::from_accept_encoding("br"), Encoding::Brotli);
        assert_eq!(Encoding::from_accept_encoding("gzip, zstd"), Encoding::Zstd);
        assert_eq!(Encoding::from_accept_encoding("zstd"), Encoding::Zstd);
        assert_eq!(Encoding::from_accept_encoding("gzip"), Encoding::Gzip);
        assert_eq!(
            Encoding::from_accept_encoding("deflate"),
            Encoding::Identity
        );
        assert_eq!(Encoding::from_accept_encoding(""), Encoding::Identity);
    }

    #[test]
    fn test_encoding_no_substring_false_positives() {
        assert_eq!(
            Encoding::from_accept_encoding("vibrant"),
            Encoding::Identity
        );
        assert_eq!(Encoding::from_accept_encoding("broken"), Encoding::Identity);
    }

    #[test]
    fn test_encoding_with_quality_values() {
        assert_eq!(
            Encoding::from_accept_encoding("gzip;q=1.0, br;q=0.8"),
            Encoding::Brotli
        );
        assert_eq!(
            Encoding::from_accept_encoding("gzip;q=0.5, zstd;q=1.0"),
            Encoding::Zstd
        );
    }

    #[test]
    fn test_encoding_respects_q_zero() {
        // q=0 means explicitly rejected
        assert_eq!(
            Encoding::from_accept_encoding("br;q=0, gzip"),
            Encoding::Gzip
        );
        assert_eq!(
            Encoding::from_accept_encoding("br;q=0, zstd;q=0, gzip"),
            Encoding::Gzip
        );
        assert_eq!(
            Encoding::from_accept_encoding("br;q=0, zstd;q=0, gzip;q=0"),
            Encoding::Identity
        );
        assert_eq!(
            Encoding::from_accept_encoding("gzip;q=0"),
            Encoding::Identity
        );
    }
}
