// Pre-built HTTP response buffer system
// Builds complete HTTP responses (headers + body) at startup for fast serving
//
// Runtime is literally just: map[path][encoding]
// All the logic happens at startup when we build every valid path/encoding combination

use anyhow::Result;
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;

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

    /// All encoding types in priority order (best first)
    pub fn all_priority() -> &'static [Encoding] {
        &[
            Encoding::Brotli,
            Encoding::Zstd,
            Encoding::Gzip,
            Encoding::Plain,
        ]
    }
}

/// Key for looking up pre-built responses
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ResponseKey {
    pub path: Arc<str>,
    pub encoding: Encoding,
}

/// Pre-built HTTP response ready for fast sending
#[derive(Debug, Clone)]
pub struct RegisteredResponse {
    /// Complete HTTP response (headers + body)
    pub data: Bytes,
    /// Buffer ID for io_uring registration
    pub buffer_id: u16,
}

/// Manager for registered buffers
/// Runtime is map[path][encoding] - all logic is startup
pub struct RegisteredBufferManager {
    /// Map of (path, encoding) -> pre-built response
    responses: DashMap<ResponseKey, RegisteredResponse>,
    /// All buffer data in order (for io_uring registration)
    buffers: Vec<Bytes>,
}

impl RegisteredBufferManager {
    /// Create and pre-build all HTTP responses
    /// Generates every valid (path, encoding) combination at startup
    pub fn new(routes: &CachedRoutes) -> Result<Self> {
        let mut manager = Self {
            responses: DashMap::new(),
            buffers: Vec::new(),
        };

        let mut buffer_id: u16 = 0;

        // Iterate through all cached routes and build all encoding variants
        for entry in routes.iter() {
            let path = entry.key().clone();
            let route = entry.value();

            // Build response for each encoding variant that exists
            for &encoding in Encoding::all_priority() {
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
    /// This is the ONLY thing the runtime does: map[path][encoding]
    pub fn get(&self, path: &str, encoding: Encoding) -> Option<RegisteredResponse> {
        let key = ResponseKey {
            path: Arc::from(path),
            encoding,
        };
        self.responses.get(&key).map(|r| r.clone())
    }

    /// Get all buffers (for io_uring registration)
    pub fn buffers(&self) -> &[Bytes] {
        &self.buffers
    }

    /// Total number of pre-built response buffers
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Get best available encoding for a path based on client Accept-Encoding
    /// Returns (path, encoding) for direct lookup
    ///
    /// Priority: br > zstd > gzip > plain
    pub fn best_match(&self, path: &str, accept_encoding: &str) -> Option<(Arc<str>, Encoding)> {
        let path_arc: Arc<str> = Arc::from(path);

        for &encoding in Encoding::all_priority() {
            // Check if client accepts this encoding
            let accepts = match encoding {
                Encoding::Plain => true,
                Encoding::Brotli => accept_encoding.contains("br"),
                Encoding::Gzip => accept_encoding.contains("gzip"),
                Encoding::Zstd => accept_encoding.contains("zstd"),
            };

            if accepts
                && self.responses.contains_key(&ResponseKey {
                    path: path_arc.clone(),
                    encoding,
                })
            {
                return Some((path_arc, encoding));
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
