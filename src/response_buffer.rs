use bytes::Bytes;
use std::sync::{Arc, LazyLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Identity,
    Gzip,
    Brotli,
    Zstd,
}

impl Encoding {
    pub const ALL: [Self; 4] = [Self::Identity, Self::Gzip, Self::Brotli, Self::Zstd];

    /// Priority: br > zstd > gzip > identity
    #[inline(always)]
    pub fn from_accept_encoding(accept: &str) -> Self {
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
}

#[derive(Debug, Clone)]
pub struct ResponseBuffer {
    pub body: Bytes,
    pub content_type: Arc<str>,
    pub content_encoding: Option<&'static str>,
    pub etag: Arc<str>,
    pub last_modified: Arc<str>,
    pub cache_control: Arc<str>,
}

// Static error responses - no allocation on error paths
static NOT_FOUND: LazyLock<ResponseBuffer> = LazyLock::new(|| ResponseBuffer {
    body: Bytes::from_static(b"Not Found"),
    content_type: Arc::from("text/plain"),
    content_encoding: None,
    etag: Arc::from("\"404\""),
    last_modified: Arc::from("Thu, 01 Jan 1970 00:00:00 GMT"),
    cache_control: Arc::from("no-cache"),
});

static BAD_REQUEST: LazyLock<ResponseBuffer> = LazyLock::new(|| ResponseBuffer {
    body: Bytes::from_static(b"Bad Request"),
    content_type: Arc::from("text/plain"),
    content_encoding: None,
    etag: Arc::from("\"400\""),
    last_modified: Arc::from("Thu, 01 Jan 1970 00:00:00 GMT"),
    cache_control: Arc::from("no-cache"),
});

impl ResponseBuffer {
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

    #[inline(always)]
    pub fn not_found() -> Self {
        NOT_FOUND.clone()
    }

    #[inline(always)]
    pub fn bad_request() -> Self {
        BAD_REQUEST.clone()
    }
}
