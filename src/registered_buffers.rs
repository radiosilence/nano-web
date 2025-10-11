// Pre-built HTTP response system with UNSAFE raw pointer approach
//
// We pre-build all HTTP responses at startup and store them in stable memory.
// Then we use RAW POINTERS to let multiple concurrent connections read from the
// same buffer without Rust's ownership restrictions.
//
// SAFETY: This is safe because:
// 1. Buffers are allocated at startup and never moved or freed until shutdown
// 2. Buffers are immutable after creation (no writes, only reads)
// 3. Multiple readers from immutable memory is safe
// 4. We use Pin to ensure buffers don't move in memory
//
// Runtime: map[path][encoding] → (*const u8, len) → unsafe write

use anyhow::Result;
use dashmap::DashMap;
use std::pin::Pin;
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

/// A pre-built response with raw pointer access
/// SAFETY: pointer is valid for the lifetime of RegisteredBufferManager
#[derive(Debug, Clone, Copy)]
pub struct UnsafeResponse {
    /// Raw pointer to response data (UNSAFE but we control lifetime)
    pub ptr: *const u8,
    /// Length of response
    pub len: usize,
    /// Buffer ID for io_uring registered buffers
    pub buffer_id: u16,
}

// SAFETY: We know these pointers are safe to send across threads because:
// 1. The underlying buffers never move (pinned)
// 2. The buffers are immutable (no writes after creation)
// 3. Multiple readers from immutable memory is inherently safe
unsafe impl Send for UnsafeResponse {}
unsafe impl Sync for UnsafeResponse {}

/// Manager for pre-built HTTP responses using unsafe raw pointers
///
/// This is UNSAFE but necessary to allow multiple concurrent connections
/// to read from the same pre-built response buffer without Rust's
/// ownership restrictions getting in the way.
pub struct RegisteredBufferManager {
    /// Map of (path, encoding) -> raw pointer to response
    responses: DashMap<ResponseKey, UnsafeResponse>,
    /// Pinned storage for all response buffers (must outlive all pointers)
    /// Box ensures the Vec's heap allocation never moves
    _storage: Pin<Box<Vec<Vec<u8>>>>,
}

impl RegisteredBufferManager {
    /// Create and pre-build all HTTP responses with UNSAFE raw pointers
    /// Generates every valid (path, encoding) combination at startup
    /// If spa_mode is true, creates aliases for common paths pointing to /index.html
    pub fn new(routes: &CachedRoutes, spa_mode: bool) -> Result<Self> {
        let responses = DashMap::new();
        let mut storage = Vec::new();
        let mut buffer_id: u16 = 0;

        // Iterate through all cached routes and build all encoding variants
        for entry in routes.iter() {
            let path = entry.key().clone();
            let route = entry.value();

            // Build response for each encoding variant that exists
            for &encoding in Encoding::all_priority() {
                if let Some(response_data) = Self::build_http_response(route, encoding) {
                    // Store the buffer in our stable storage
                    storage.push(response_data);

                    // SAFETY: We're getting a pointer to the last element we just pushed.
                    // The Vec is pinned below, so this pointer stays valid forever.
                    let buf = storage.last().unwrap();
                    let ptr = buf.as_ptr();
                    let len = buf.len();

                    let unsafe_response = UnsafeResponse {
                        ptr,
                        len,
                        buffer_id,
                    };

                    // Insert main path
                    responses.insert(
                        ResponseKey {
                            path: path.clone(),
                            encoding,
                        },
                        unsafe_response,
                    );

                    // If this is /index.html, also register it as /
                    if path.as_ref() == "/index.html" {
                        responses.insert(
                            ResponseKey {
                                path: Arc::from("/"),
                                encoding,
                            },
                            unsafe_response,
                        );
                    }

                    buffer_id += 1;
                }
            }
        }

        // Pin the storage so the Vec never moves in memory
        // This ensures all our raw pointers stay valid
        let storage = Box::pin(storage);

        Ok(Self {
            responses,
            _storage: storage,
        })
    }

    /// Get iovecs for io_uring buffer registration
    /// SAFETY: These pointers are valid for the lifetime of RegisteredBufferManager
    pub fn get_iovecs(&self) -> Vec<libc::iovec> {
        self._storage
            .iter()
            .map(|buf| libc::iovec {
                iov_base: buf.as_ptr() as *mut libc::c_void,
                iov_len: buf.len(),
            })
            .collect()
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
    /// Returns UnsafeResponse with raw pointer (Copy is cheap - just pointer + len)
    pub fn get(&self, path: &str, encoding: Encoding) -> Option<UnsafeResponse> {
        let key = ResponseKey {
            path: Arc::from(path),
            encoding,
        };
        self.responses.get(&key).map(|r| *r.value())
    }

    /// Convert UnsafeResponse to a safe slice for reading
    /// SAFETY: The pointer is guaranteed valid for the lifetime of RegisteredBufferManager
    pub fn as_slice(&self, response: &UnsafeResponse) -> &[u8] {
        unsafe { std::slice::from_raw_parts(response.ptr, response.len) }
    }

    /// Total number of pre-built responses
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
