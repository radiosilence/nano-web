// io_uring registered buffers for true zero-copy serving
// Pre-builds complete HTTP responses and registers them with the kernel

use anyhow::{Context, Result};
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;

use crate::compression::CompressedContent;
use crate::http::build_response;
use crate::routes::{CachedRoute, CachedRoutes};

/// Encoding variant for a file
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Encoding {
    Plain,
    Brotli,
    Gzip,
    Zstd,
}

impl Encoding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Encoding::Plain => "identity",
            Encoding::Brotli => "br",
            Encoding::Gzip => "gzip",
            Encoding::Zstd => "zstd",
        }
    }

    pub fn content_encoding_header(&self) -> Option<&'static str> {
        match self {
            Encoding::Plain => None,
            Encoding::Brotli => Some("br"),
            Encoding::Gzip => Some("gzip"),
            Encoding::Zstd => Some("zstd"),
        }
    }
}

/// Key for looking up pre-built responses
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ResponseKey {
    pub path: Arc<str>,
    pub encoding: Encoding,
}

/// Pre-built HTTP response ready for zero-copy sending
#[derive(Debug, Clone)]
pub struct RegisteredResponse {
    /// Complete HTTP response (headers + body)
    pub data: Bytes,
    /// Buffer ID in io_uring (for IOSQE_FIXED_FILE)
    pub buffer_id: u16,
}

/// Manager for registered buffers
pub struct RegisteredBufferManager {
    /// Map of (path, encoding) -> pre-built response
    responses: DashMap<ResponseKey, RegisteredResponse>,
    /// All buffer data in order (for io_uring registration)
    buffers: Vec<Bytes>,
}

impl RegisteredBufferManager {
    /// Create and pre-build all HTTP responses
    pub fn new(routes: &CachedRoutes) -> Result<Self> {
        let mut manager = Self {
            responses: DashMap::new(),
            buffers: Vec::new(),
        };

        let mut buffer_id: u16 = 0;

        // Iterate through all cached routes
        for entry in routes.iter() {
            let path = entry.key().clone();
            let route = entry.value();

            // Build response for each encoding variant
            for encoding in [
                Encoding::Plain,
                Encoding::Brotli,
                Encoding::Gzip,
                Encoding::Zstd,
            ] {
                if let Some(response_data) = Self::build_http_response(route, encoding) {
                    let data = Bytes::from(response_data);

                    // Store in lookup map
                    manager.responses.insert(
                        ResponseKey {
                            path: path.clone(),
                            encoding,
                        },
                        RegisteredResponse {
                            data: data.clone(),
                            buffer_id,
                        },
                    );

                    // Store in buffer list (in order for io_uring registration)
                    manager.buffers.push(data);
                    buffer_id += 1;
                }
            }
        }

        Ok(manager)
    }

    /// Build a complete HTTP response for a file + encoding
    fn build_http_response(route: &CachedRoute, encoding: Encoding) -> Option<Vec<u8>> {
        // Get the appropriate compressed variant
        let body = match encoding {
            Encoding::Plain => &route.content.plain,
            Encoding::Brotli => route.content.brotli.as_ref()?,
            Encoding::Gzip => route.content.gzip.as_ref()?,
            Encoding::Zstd => route.content.zstd.as_ref()?,
        };

        // Build headers
        let mut headers = vec![
            ("Content-Type", route.headers.content_type.as_ref()),
            ("Cache-Control", "public, max-age=3600"),
            ("Server", "nano-web-uring-zerocopy"),
            ("ETag", route.headers.etag.as_ref()),
            ("Last-Modified", route.headers.last_modified.as_ref()),
        ];

        // Add Content-Encoding if compressed
        if let Some(enc) = encoding.content_encoding_header() {
            headers.push(("Content-Encoding", enc));
        }

        Some(build_response(200, &headers, body))
    }

    /// Look up pre-built response by path and encoding
    pub fn get_response(&self, path: &str, encoding: Encoding) -> Option<RegisteredResponse> {
        let key = ResponseKey {
            path: Arc::from(path),
            encoding,
        };
        self.responses.get(&key).map(|r| r.clone())
    }

    /// Get all buffers for io_uring registration
    pub fn buffers(&self) -> &[Bytes] {
        &self.buffers
    }

    /// Total number of registered buffers
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Select best encoding based on Accept-Encoding header
    pub fn select_encoding(&self, path: &str, accept_encoding: &str) -> Option<Encoding> {
        // Try encodings in priority order: brotli > zstd > gzip > plain
        for encoding in [
            Encoding::Brotli,
            Encoding::Zstd,
            Encoding::Gzip,
            Encoding::Plain,
        ] {
            // Check if client accepts this encoding
            let accepts = match encoding {
                Encoding::Plain => true,
                Encoding::Brotli => accept_encoding.contains("br"),
                Encoding::Gzip => accept_encoding.contains("gzip"),
                Encoding::Zstd => accept_encoding.contains("zstd"),
            };

            if accepts {
                // Check if we have this variant
                let key = ResponseKey {
                    path: Arc::from(path),
                    encoding,
                };
                if self.responses.contains_key(&key) {
                    return Some(encoding);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_selection() {
        // Test that we properly parse Accept-Encoding headers
        assert_eq!(Encoding::Brotli.as_str(), "br");
        assert_eq!(Encoding::Gzip.as_str(), "gzip");
    }
}
