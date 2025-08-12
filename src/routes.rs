use crate::compression::CompressedContent;
use crate::mime_types::{get_cache_control, get_mime_config};
use crate::template::render_template;
use anyhow::Result;
use dashmap::DashMap;
use fxhash::FxBuildHasher;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, error, info};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CachedRoute {
    pub content: Arc<CompressedContent>,
    pub path: Arc<PathBuf>,
    pub modified: SystemTime,
    pub headers: Arc<CachedRouteHeaders>,
}

#[derive(Debug, Clone)]
pub struct CachedRouteHeaders {
    pub content_type: Arc<str>,
    pub last_modified: Arc<str>,
    pub etag: Arc<str>,
    pub cache_control: Arc<str>,
}

pub type CachedRoutes = DashMap<Arc<str>, CachedRoute, FxBuildHasher>;

pub struct NanoWeb {
    pub routes: CachedRoutes,
    pub static_cache: DashMap<Arc<str>, Arc<Mmap>, FxBuildHasher>,
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
            static_cache: DashMap::with_hasher(FxBuildHasher::default()),
        }
    }

    pub fn populate_routes(&self, public_dir: &Path, config_prefix: &str) -> Result<()> {
        debug!("Starting route population from {:?}", public_dir);

        // Collect all file paths first
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

        // Process files in parallel
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

        // Insert routes into concurrent map
        for (url_path, route) in routes {
            self.routes.insert(url_path.clone(), route.clone());

            // Handle index files
            if url_path.ends_with("/index.html") {
                let dir_path = if url_path.as_ref() == "/index.html" {
                    Arc::from("/")
                } else {
                    let dir = url_path.trim_end_matches("/index.html");
                    Arc::from(format!("{}/", dir))
                };
                self.routes.insert(dir_path, route);
            }
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
        // Memory-map large files for zero-copy serving
        let content = if metadata.len() > 8192 {
            // Use memory mapping for larger files
            let file = File::open(file_path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            let url_path = self.file_path_to_url(file_path, public_dir)?;
            self.static_cache.insert(url_path.clone(), Arc::new(mmap));

            // For mmap files, we still need a copy for compression
            std::fs::read(file_path)?
        } else {
            std::fs::read(file_path)?
        };

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
        let etag = self.generate_fast_etag(&modified, &compressed.plain);
        let last_modified = self.format_fast_http_date(modified);

        let headers = Arc::new(CachedRouteHeaders {
            content_type: Arc::from(mime_config.mime_type.as_str()),
            last_modified: Arc::from(last_modified.as_str()),
            etag: Arc::from(etag.as_str()),
            cache_control: Arc::from(get_cache_control(&mime_config.mime_type)),
        });

        let route = CachedRoute {
            content: Arc::new(compressed),
            path: Arc::new(file_path.to_path_buf()),
            modified,
            headers,
        };

        let url_path = self.file_path_to_url(file_path, public_dir)?;
        Ok((url_path, route))
    }

    #[inline(always)]
    pub fn get_route(&self, path: &str) -> Option<CachedRoute> {
        self.routes.get(path).map(|entry| entry.value().clone())
    }

    fn file_path_to_url(&self, file_path: &Path, public_dir: &Path) -> Result<Arc<str>> {
        let relative = file_path.strip_prefix(public_dir)?;
        let url_path = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
        Ok(Arc::from(url_path.as_str()))
    }

    fn generate_fast_etag(&self, modified: &SystemTime, content: &[u8]) -> String {
        use std::time::UNIX_EPOCH;

        let timestamp = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Simple ETag: timestamp-size
        format!("\"{:x}-{:x}\"", timestamp, content.len())
    }

    fn format_fast_http_date(&self, time: SystemTime) -> String {
        let datetime = chrono::DateTime::<chrono::Utc>::from(time);
        datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
    }
}

// Lock-free atomic operations for route updates in dev mode
impl NanoWeb {
    pub fn refresh_if_modified(
        &self,
        path: &str,
        config_prefix: &str,
    ) -> Result<Option<CachedRoute>> {
        if let Some(route_ref) = self.routes.get(path) {
            let route = route_ref.value().clone();
            drop(route_ref); // Release the reference early

            let metadata = std::fs::metadata(&*route.path)?;
            let modified = metadata.modified()?;

            if modified > route.modified {
                debug!("File modified, refreshing route: {:?}", route.path);

                // Create new route
                let parent_dir = route.path.parent().unwrap();
                let public_dir = parent_dir.ancestors().last().unwrap();
                let (_, new_route) =
                    self.create_route(&route.path, &metadata, public_dir, config_prefix)?;

                // Atomic update
                self.routes.insert(Arc::from(path), new_route.clone());
                return Ok(Some(new_route));
            }
            Ok(Some(route))
        } else {
            Ok(None)
        }
    }
}
