use std::sync::Arc;

/// Encoding for lookup key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Identity,
    Gzip,
    Brotli,
    Zstd,
}

impl Encoding {
    /// Parse from Accept-Encoding header, returns best available
    pub fn from_accept_encoding(accept: &str) -> Self {
        // Priority: br > zstd > gzip > identity
        if accept.contains("br") {
            Self::Brotli
        } else if accept.contains("zstd") {
            Self::Zstd
        } else if accept.contains("gzip") {
            Self::Gzip
        } else {
            Self::Identity
        }
    }

    pub fn header_value(&self) -> Option<&'static str> {
        match self {
            Self::Identity => None,
            Self::Gzip => Some("gzip"),
            Self::Brotli => Some("br"),
            Self::Zstd => Some("zstd"),
        }
    }
}

/// Pre-baked HTTP response - complete buffer ready to blast onto socket
#[derive(Debug, Clone)]
pub struct ResponseBuffer {
    /// Complete HTTP response: status line + headers + \r\n\r\n + body
    /// This is literally what goes on the wire, no further processing
    pub buffer: Arc<Vec<u8>>,
}

impl ResponseBuffer {
    /// Build complete HTTP response from file metadata and compressed body
    pub fn build(
        content_type: &str,
        encoding: Encoding,
        etag: &str,
        last_modified: &str,
        cache_control: &str,
        body: &[u8],
    ) -> Self {
        let mut buf = Vec::with_capacity(512 + body.len());

        // Status line
        buf.extend_from_slice(b"HTTP/1.1 200 OK\r\n");

        // Headers
        buf.extend_from_slice(b"Content-Type: ");
        buf.extend_from_slice(content_type.as_bytes());
        buf.extend_from_slice(b"\r\n");

        if let Some(enc) = encoding.header_value() {
            buf.extend_from_slice(b"Content-Encoding: ");
            buf.extend_from_slice(enc.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }

        buf.extend_from_slice(b"Content-Length: ");
        buf.extend_from_slice(body.len().to_string().as_bytes());
        buf.extend_from_slice(b"\r\n");

        buf.extend_from_slice(b"ETag: ");
        buf.extend_from_slice(etag.as_bytes());
        buf.extend_from_slice(b"\r\n");

        buf.extend_from_slice(b"Last-Modified: ");
        buf.extend_from_slice(last_modified.as_bytes());
        buf.extend_from_slice(b"\r\n");

        buf.extend_from_slice(b"Cache-Control: ");
        buf.extend_from_slice(cache_control.as_bytes());
        buf.extend_from_slice(b"\r\n");

        // Security headers
        buf.extend_from_slice(b"X-Content-Type-Options: nosniff\r\n");
        buf.extend_from_slice(b"X-Frame-Options: SAMEORIGIN\r\n");
        buf.extend_from_slice(b"Referrer-Policy: strict-origin-when-cross-origin\r\n");

        // End of headers
        buf.extend_from_slice(b"\r\n");

        // Body
        buf.extend_from_slice(body);

        Self {
            buffer: Arc::new(buf),
        }
    }

    /// Build a complete HTTP/1.1 response buffer from scratch (legacy)
    pub fn new(status_code: u16, status_text: &str, headers: &[(&str, &str)], body: &[u8]) -> Self {
        // Pre-allocate: status line ~15 bytes + headers ~200 bytes + body
        let mut buffer = Vec::with_capacity(256 + body.len());

        // Status line
        buffer.extend_from_slice(b"HTTP/1.1 ");
        buffer.extend_from_slice(status_code.to_string().as_bytes());
        buffer.extend_from_slice(b" ");
        buffer.extend_from_slice(status_text.as_bytes());
        buffer.extend_from_slice(b"\r\n");

        // Headers
        for (key, value) in headers {
            buffer.extend_from_slice(key.as_bytes());
            buffer.extend_from_slice(b": ");
            buffer.extend_from_slice(value.as_bytes());
            buffer.extend_from_slice(b"\r\n");
        }

        // Blank line separating headers from body
        buffer.extend_from_slice(b"\r\n");

        // Body
        buffer.extend_from_slice(body);

        Self {
            buffer: Arc::new(buffer),
        }
    }

    /// Build 404 response
    pub fn not_found() -> Self {
        Self::new(
            404,
            "Not Found",
            &[
                ("Content-Type", "text/plain"),
                ("Content-Length", "9"),
                ("Cache-Control", "no-cache"),
            ],
            b"Not Found",
        )
    }

    /// Build 400 response
    pub fn bad_request() -> Self {
        Self::new(
            400,
            "Bad Request",
            &[
                ("Content-Type", "text/plain"),
                ("Content-Length", "11"),
                ("Cache-Control", "no-cache"),
            ],
            b"Bad Request",
        )
    }

    /// Build 500 response
    pub fn internal_error() -> Self {
        Self::new(
            500,
            "Internal Server Error",
            &[
                ("Content-Type", "text/plain"),
                ("Content-Length", "21"),
                ("Cache-Control", "no-cache"),
            ],
            b"Internal Server Error",
        )
    }
}
