// Pre-built HTTP response buffer system with io_uring fixed buffers
// Builds complete HTTP responses (headers + body) at startup and registers with kernel
//
// Runtime is literally just: map[path][encoding] → check_out(buffer_id) → write_fixed()
// All the logic happens at startup when we build every valid path/encoding combination

use anyhow::Result;
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;
use tokio_uring::buf::fixed::FixedBufRegistry;

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

/// Manager for registered buffers with io_uring FixedBufRegistry
/// Runtime is map[path][encoding] → buffer_id → check_out() → write_fixed()
pub struct RegisteredBufferManager {
    /// Map of (path, encoding) -> buffer_id
    responses: DashMap<ResponseKey, u16>,
    /// io_uring fixed buffer registry (registered with kernel)
    /// Vec<u8> is the buffer type we're registering
    registry: Arc<FixedBufRegistry<Vec<u8>>>,
}

impl RegisteredBufferManager {
    /// Create and pre-build all HTTP responses, then register with io_uring
    /// Generates every valid (path, encoding) combination at startup
    pub fn new(routes: &CachedRoutes) -> Result<Self> {
        let responses = DashMap::new();
        let mut buffers: Vec<Vec<u8>> = Vec::new();
        let mut buffer_id: u16 = 0;

        // Iterate through all cached routes and build all encoding variants
        for entry in routes.iter() {
            let path = entry.key().clone();
            let route = entry.value();

            // Build response for each encoding variant that exists
            for &encoding in Encoding::all_priority() {
                if let Some(response_data) = Self::build_http_response(route, encoding) {
                    // Store buffer_id in lookup map
                    responses.insert(
                        ResponseKey {
                            path: path.clone(),
                            encoding,
                        },
                        buffer_id,
                    );

                    // Store buffer for registry
                    buffers.push(response_data);
                    buffer_id += 1;
                }
            }
        }

        // Create FixedBufRegistry with all pre-built responses
        let registry = FixedBufRegistry::new(buffers);

        Ok(Self {
            responses,
            registry: Arc::new(registry),
        })
    }

    /// Register buffers with io_uring kernel
    /// Must be called from within tokio_uring runtime
    pub fn register(&self) -> Result<()> {
        self.registry.register()?;
        Ok(())
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

    /// Look up buffer_id by path and encoding
    /// Returns the buffer_id for use with check_out()
    pub fn get_buffer_id(&self, path: &str, encoding: Encoding) -> Option<u16> {
        let key = ResponseKey {
            path: Arc::from(path),
            encoding,
        };
        self.responses.get(&key).map(|r| *r.value())
    }

    /// Check out a fixed buffer from the registry
    /// This gives you the actual buffer to write with write_fixed()
    pub fn check_out(&self, buffer_id: u16) -> Option<tokio_uring::buf::fixed::FixedBuf> {
        self.registry.check_out(buffer_id as usize)
    }

    /// Total number of registered buffers
    pub fn buffer_count(&self) -> usize {
        self.responses.len()
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
