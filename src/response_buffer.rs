use bytes::Bytes;
use std::sync::Arc;

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
        Self {
            body,
            content_type,
            content_encoding,
            etag,
            last_modified,
            cache_control,
            content_length,
            vary_encoding,
        }
    }
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
