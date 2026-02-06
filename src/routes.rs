use crate::compression::CompressedContent;
use crate::mime_types::{get_cache_control, get_mime_config};
use crate::response_buffer::{Encoding, ResponseBuffer};
use crate::template::render_template;
use anyhow::Result;
use dashmap::DashMap;
use fxhash::FxBuildHasher;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, error, info};
use walkdir::WalkDir;

type RouteMap = DashMap<Arc<str>, ResponseBuffer, FxBuildHasher>;

#[derive(Debug, Clone)]
struct CachedRoute {
    content: Arc<CompressedContent>,
    path: Arc<PathBuf>,
    modified: SystemTime,
}

type CachedRoutes = DashMap<Arc<str>, CachedRoute, FxBuildHasher>;

/// Pre-baked responses per encoding. Separate maps enable &str lookup against Arc<str> keys.
struct ResponseCache {
    identity: RouteMap,
    gzip: RouteMap,
    brotli: RouteMap,
    zstd: RouteMap,
}

impl ResponseCache {
    fn new() -> Self {
        Self {
            identity: DashMap::with_hasher(FxBuildHasher::default()),
            gzip: DashMap::with_hasher(FxBuildHasher::default()),
            brotli: DashMap::with_hasher(FxBuildHasher::default()),
            zstd: DashMap::with_hasher(FxBuildHasher::default()),
        }
    }

    #[inline(always)]
    fn get_map(&self, encoding: Encoding) -> &RouteMap {
        match encoding {
            Encoding::Identity => &self.identity,
            Encoding::Gzip => &self.gzip,
            Encoding::Brotli => &self.brotli,
            Encoding::Zstd => &self.zstd,
        }
    }

    #[inline(always)]
    fn get(&self, path: &str, encoding: Encoding) -> Option<ResponseBuffer> {
        // Try requested encoding first, fallback to identity for non-compressible files
        self.get_map(encoding)
            .get(path)
            .or_else(|| self.identity.get(path))
            .map(|e| e.value().clone())
    }

    fn insert(&self, path: Arc<str>, encoding: Encoding, buf: ResponseBuffer) {
        self.get_map(encoding).insert(path, buf);
    }
}

pub struct NanoWeb {
    routes: CachedRoutes,
    responses: ResponseCache,
}

impl Default for NanoWeb {
    fn default() -> Self {
        Self::new()
    }
}

impl NanoWeb {
    pub fn new() -> Self {
        Self {
            routes: DashMap::with_hasher(FxBuildHasher::default()),
            responses: ResponseCache::new(),
        }
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    #[inline(always)]
    pub fn get_response(&self, path: &str, accept_encoding: &str) -> Option<ResponseBuffer> {
        let encoding = Encoding::from_accept_encoding(accept_encoding);
        self.responses.get(path, encoding)
    }

    pub fn populate_routes(&self, public_dir: &Path, config_prefix: &str) -> Result<()> {
        debug!("Starting route population from {:?}", public_dir);

        let file_paths: Vec<_> = WalkDir::new(public_dir)
            .into_iter()
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    if e.file_type().is_file() {
                        Some((e.path().to_path_buf(), e.metadata().ok()?))
                    } else {
                        None
                    }
                })
            })
            .collect();

        info!("Processing {} files in parallel", file_paths.len());

        let routes: Vec<_> = file_paths
            .par_iter()
            .filter_map(|(file_path, metadata)| {
                match self.create_route(file_path, metadata, public_dir, config_prefix) {
                    Ok((url_path, route)) => Some((url_path, route)),
                    Err(e) => {
                        error!("Failed to create route for {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .collect();

        for (url_path, route) in routes {
            // Handle index files - create directory alias
            if url_path.ends_with("/index.html") {
                let dir_path: Arc<str> = if url_path.as_ref() == "/index.html" {
                    Arc::from("/")
                } else {
                    let dir = url_path.trim_end_matches("/index.html");
                    Arc::from(format!("{}/", dir).as_str())
                };
                self.routes.insert(dir_path.clone(), route.clone());

                for encoding in Encoding::ALL {
                    if let Some(response) = self.responses.get(url_path.as_ref(), encoding) {
                        self.responses.insert(dir_path.clone(), encoding, response);
                    }
                }
            }

            self.routes.insert(url_path, route);
        }

        info!("Routes populated: {} routes", self.routes.len());
        Ok(())
    }

    fn create_route(
        &self,
        file_path: &Path,
        metadata: &std::fs::Metadata,
        public_dir: &Path,
        config_prefix: &str,
    ) -> Result<(Arc<str>, CachedRoute)> {
        let content = std::fs::read(file_path)?;

        let modified = metadata.modified()?;
        let mime_config = get_mime_config(file_path);

        // Apply templating if needed
        let processed_content = if mime_config.is_templatable {
            match render_template(&String::from_utf8_lossy(&content), config_prefix) {
                Ok(templated) => templated.into_bytes(),
                Err(e) => {
                    error!("Template rendering failed for {:?}: {}", file_path, e);
                    content
                }
            }
        } else {
            content
        };

        let compressed = CompressedContent::new(processed_content, mime_config.is_compressible)?;
        let etag = Self::generate_etag(&modified, &compressed.plain);
        let last_modified = Self::format_http_date(modified);

        let ct: Arc<str> = Arc::from(mime_config.mime_type.as_str());
        let etag: Arc<str> = Arc::from(etag.as_str());
        let lm: Arc<str> = Arc::from(last_modified.as_str());
        let cc: Arc<str> = Arc::from(get_cache_control(&mime_config.mime_type));

        let route = CachedRoute {
            content: Arc::new(compressed),
            path: Arc::new(file_path.to_path_buf()),
            modified,
        };

        let url_path = Self::file_path_to_url(file_path, public_dir)?;

        self.responses.insert(
            url_path.clone(),
            Encoding::Identity,
            ResponseBuffer::new(
                route.content.plain.to_vec(),
                ct.clone(),
                None,
                etag.clone(),
                lm.clone(),
                cc.clone(),
            ),
        );

        if let Some(data) = &route.content.gzip {
            self.responses.insert(
                url_path.clone(),
                Encoding::Gzip,
                ResponseBuffer::new(
                    data.to_vec(),
                    ct.clone(),
                    Some("gzip"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                ),
            );
        }

        if let Some(data) = &route.content.brotli {
            self.responses.insert(
                url_path.clone(),
                Encoding::Brotli,
                ResponseBuffer::new(
                    data.to_vec(),
                    ct.clone(),
                    Some("br"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                ),
            );
        }

        if let Some(data) = &route.content.zstd {
            self.responses.insert(
                url_path.clone(),
                Encoding::Zstd,
                ResponseBuffer::new(
                    data.to_vec(),
                    ct.clone(),
                    Some("zstd"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                ),
            );
        }

        Ok((url_path, route))
    }

    fn file_path_to_url(file_path: &Path, public_dir: &Path) -> Result<Arc<str>> {
        let relative = file_path.strip_prefix(public_dir)?;
        let url_path = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
        Ok(Arc::from(url_path.as_str()))
    }

    fn generate_etag(modified: &SystemTime, content: &[u8]) -> String {
        use std::time::UNIX_EPOCH;
        let timestamp = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("\"{:x}-{:x}\"", timestamp, content.len())
    }

    fn format_http_date(time: SystemTime) -> String {
        let datetime = chrono::DateTime::<chrono::Utc>::from(time);
        datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
    }
}

impl NanoWeb {
    /// Dev mode: refresh route if file changed on disk
    pub fn refresh_if_modified(
        &self,
        url_path: &str,
        public_dir: &Path,
        config_prefix: &str,
    ) -> Result<bool> {
        let Some(route_ref) = self.routes.get(url_path) else {
            return Ok(false);
        };
        let route = route_ref.value().clone();
        drop(route_ref);

        let metadata = std::fs::metadata(&*route.path)?;
        if metadata.modified()? > route.modified {
            debug!("File modified, refreshing: {:?}", route.path);
            let (new_url, new_route) =
                self.create_route(&route.path, &metadata, public_dir, config_prefix)?;
            self.routes.insert(new_url, new_route);
            return Ok(true);
        }
        Ok(false)
    }
}
