use bytes::Bytes;
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

/// Pre-baked response buffer: body + metadata for hyper
/// The body is pre-compressed, headers are pre-computed strings
/// Hyper serializes headers, we provide zero-allocation body via Bytes
#[derive(Debug, Clone)]
pub struct ResponseBuffer {
    /// Pre-compressed body (Bytes wraps Arc<Vec<u8>> - zero copy on clone)
    pub body: Bytes,

    /// Pre-computed header values
    pub content_type: Arc<str>,
    pub content_encoding: Option<&'static str>,
    pub etag: Arc<str>,
    pub last_modified: Arc<str>,
    pub cache_control: Arc<str>,
}

impl ResponseBuffer {
    /// Create response buffer from pre-compressed body and metadata
    pub fn new(
        body: Vec<u8>,
        content_type: Arc<str>,
        content_encoding: Option<&'static str>,
        etag: Arc<str>,
        last_modified: Arc<str>,
        cache_control: Arc<str>,
    ) -> Self {
        Self {
            body: Bytes::from(body),
            content_type,
            content_encoding,
            etag,
            last_modified,
            cache_control,
        }
    }

    /// Static 404 response
    pub fn not_found() -> Self {
        Self::new(
            b"Not Found".to_vec(),
            Arc::from("text/plain"),
            None,
            Arc::from("\"404\""),
            Arc::from("Mon, 01 Jan 2024 00:00:00 GMT"),
            Arc::from("no-cache"),
        )
    }

    /// Static 400 response
    pub fn bad_request() -> Self {
        Self::new(
            b"Bad Request".to_vec(),
            Arc::from("text/plain"),
            None,
            Arc::from("\"400\""),
            Arc::from("Mon, 01 Jan 2024 00:00:00 GMT"),
            Arc::from("no-cache"),
        )
    }
}
