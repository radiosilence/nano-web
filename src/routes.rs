use crate::compression::CompressedContent;
use crate::mime_types::{get_cache_control, get_mime_config};
use crate::response_buffer::{Encoding, ResponseBuffer};
use crate::template::render_template;
use anyhow::Result;
use dashmap::DashMap;
use rayon::prelude::*;
use rustc_hash::FxBuildHasher;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, error, info};
use walkdir::WalkDir;

// Values are Arc so the hot path clones one refcount, not the whole struct (body +
// 5 Arc<str> + a 13-entry HeaderMap). Routes are write-once at startup / dev-reload.
type RouteMap = DashMap<Arc<str>, Arc<ResponseBuffer>, FxBuildHasher>;

#[derive(Debug, Clone)]
struct CachedRoute {
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
            identity: DashMap::with_hasher(FxBuildHasher),
            gzip: DashMap::with_hasher(FxBuildHasher),
            brotli: DashMap::with_hasher(FxBuildHasher),
            zstd: DashMap::with_hasher(FxBuildHasher),
        }
    }

    fn get_map(&self, encoding: Encoding) -> &RouteMap {
        match encoding {
            Encoding::Identity => &self.identity,
            Encoding::Gzip => &self.gzip,
            Encoding::Brotli => &self.brotli,
            Encoding::Zstd => &self.zstd,
        }
    }

    fn get(&self, path: &str, encoding: Encoding) -> Option<Arc<ResponseBuffer>> {
        // Try requested encoding first, fallback to identity for non-compressible files
        self.get_map(encoding)
            .get(path)
            .or_else(|| self.identity.get(path))
            .map(|e| Arc::clone(e.value()))
    }

    fn insert(&self, path: Arc<str>, encoding: Encoding, buf: Arc<ResponseBuffer>) {
        self.get_map(encoding).insert(path, buf);
    }

    fn remove(&self, path: &str) {
        for encoding in Encoding::ALL {
            self.get_map(encoding).remove(path);
        }
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
            routes: DashMap::with_hasher(FxBuildHasher),
            responses: ResponseCache::new(),
        }
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn get_response(&self, path: &str, accept_encoding: &str) -> Option<Arc<ResponseBuffer>> {
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
            self.register_route(url_path, route);
        }

        info!("Routes populated: {} routes", self.routes.len());
        Ok(())
    }

    /// The directory URL an index file is also reachable under: `/index.html`
    /// serves `/`, `/sub/index.html` serves `/sub/`.
    fn dir_alias(url_path: &str) -> Option<Arc<str>> {
        if !url_path.ends_with("/index.html") {
            return None;
        }
        Some(if url_path == "/index.html" {
            Arc::from("/")
        } else {
            let dir = url_path.trim_end_matches("/index.html");
            Arc::from(format!("{dir}/").as_str())
        })
    }

    /// Publishes a route under its URL and any directory alias. The startup
    /// scan and the dev-mode refresh both go through here — when they built
    /// routes separately, the alias kept serving the copy captured at startup.
    fn register_route(&self, url_path: Arc<str>, route: CachedRoute) {
        if let Some(dir_path) = Self::dir_alias(&url_path) {
            self.routes.insert(dir_path.clone(), route.clone());

            for encoding in Encoding::ALL {
                if let Some(response) = self.responses.get(url_path.as_ref(), encoding) {
                    self.responses.insert(dir_path.clone(), encoding, response);
                }
            }
        }

        self.routes.insert(url_path, route);
    }

    fn unregister_route(&self, url_path: &str) {
        if let Some(dir_path) = Self::dir_alias(url_path) {
            self.routes.remove(dir_path.as_ref());
            self.responses.remove(dir_path.as_ref());
        }

        self.routes.remove(url_path);
        self.responses.remove(url_path);
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
        let etag = Self::generate_etag(&compressed.plain);
        let last_modified = Self::format_http_date(modified);

        let ct: Arc<str> = Arc::from(mime_config.mime_type.as_str());
        let etag: Arc<str> = Arc::from(etag.as_str());
        let lm: Arc<str> = Arc::from(last_modified.as_str());
        let cc: Arc<str> = Arc::from(get_cache_control(&mime_config.mime_type));
        let vary = mime_config.is_compressible;

        let route = CachedRoute {
            path: Arc::new(file_path.to_path_buf()),
            modified,
        };

        let url_path = Self::file_path_to_url(file_path, public_dir)?;

        self.responses.insert(
            url_path.clone(),
            Encoding::Identity,
            Arc::new(ResponseBuffer::new(
                compressed.plain.clone(),
                ct.clone(),
                None,
                etag.clone(),
                lm.clone(),
                cc.clone(),
                vary,
            )),
        );

        if let Some(data) = &compressed.gzip {
            self.responses.insert(
                url_path.clone(),
                Encoding::Gzip,
                Arc::new(ResponseBuffer::new(
                    data.clone(),
                    ct.clone(),
                    Some("gzip"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                    vary,
                )),
            );
        }

        if let Some(data) = &compressed.brotli {
            self.responses.insert(
                url_path.clone(),
                Encoding::Brotli,
                Arc::new(ResponseBuffer::new(
                    data.clone(),
                    ct.clone(),
                    Some("br"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                    vary,
                )),
            );
        }

        if let Some(data) = &compressed.zstd {
            self.responses.insert(
                url_path.clone(),
                Encoding::Zstd,
                Arc::new(ResponseBuffer::new(
                    data.clone(),
                    ct.clone(),
                    Some("zstd"),
                    etag.clone(),
                    lm.clone(),
                    cc.clone(),
                    vary,
                )),
            );
        }

        Ok((url_path, route))
    }

    fn file_path_to_url(file_path: &Path, public_dir: &Path) -> Result<Arc<str>> {
        let relative = file_path.strip_prefix(public_dir)?;
        let url_path = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
        Ok(Arc::from(url_path.as_str()))
    }

    fn generate_etag(content: &[u8]) -> String {
        let mut hasher = rustc_hash::FxHasher::default();
        hasher.write(content);
        format!("\"{:x}\"", hasher.finish())
    }

    fn format_http_date(time: SystemTime) -> String {
        httpdate::fmt_http_date(time)
    }

    /// Maps a request path to the file that should back it, applying the same
    /// index-file convention as the startup scan. Callers pass a path that has
    /// already been through `validate_request_path`, which rejects traversal.
    fn resolve_on_disk(url_path: &str, public_dir: &Path) -> Option<PathBuf> {
        let direct = public_dir.join(url_path.trim_start_matches('/'));
        if direct.is_file() {
            return Some(direct);
        }

        let index = direct.join("index.html");
        index.is_file().then_some(index)
    }

    /// Dev mode: bring the route for `url_path` back in line with the disk,
    /// covering files that were edited, created or deleted since startup.
    pub fn refresh_if_modified(
        &self,
        url_path: &str,
        public_dir: &Path,
        config_prefix: &str,
    ) -> Result<bool> {
        // Prefer the file already behind this URL, so a route survives long
        // enough to notice its file was deleted. Requests arrive with the
        // trailing slash stripped, so `/sub` must also match the `/sub/` key
        // an index file is published under.
        let dir_key = format!("{}/", url_path.trim_end_matches('/'));
        let known = self
            .routes
            .get(url_path)
            .or_else(|| self.routes.get(dir_key.as_str()))
            .map(|r| (*r.value().path).clone());

        // Nothing cached — resolve from disk so files created after startup
        // are picked up.
        let Some(file_path) = known.or_else(|| Self::resolve_on_disk(url_path, public_dir)) else {
            return Ok(false);
        };

        let canonical = Self::file_path_to_url(&file_path, public_dir)?;

        let metadata = match std::fs::metadata(&file_path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !self.routes.contains_key(canonical.as_ref()) {
                    return Ok(false);
                }
                debug!("File removed, dropping route: {:?}", file_path);
                self.unregister_route(&canonical);
                return Ok(true);
            }
            Err(e) => return Err(e.into()),
        };

        if !metadata.is_file() {
            return Ok(false);
        }

        let modified = metadata.modified()?;
        let cached = self
            .routes
            .get(canonical.as_ref())
            .map(|r| r.value().modified);
        if cached.is_some_and(|cached| modified <= cached) {
            return Ok(false);
        }

        debug!("Refreshing route: {:?}", file_path);
        let (new_url, new_route) =
            self.create_route(&file_path, &metadata, public_dir, config_prefix)?;
        self.register_route(new_url, new_route);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requests arrive here already sanitized, which strips the trailing slash:
    /// a browser asking for `/sub/` reaches `refresh_if_modified` as `/sub`.
    fn as_requested(url: &str) -> &str {
        url.strip_suffix('/')
            .filter(|s| !s.is_empty())
            .unwrap_or(url)
    }

    fn body(server: &NanoWeb, url: &str) -> Option<String> {
        server
            .get_response(url, "")
            .map(|b| String::from_utf8_lossy(&b.body).into_owned())
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
        // Bump the mtime past whatever was cached rather than sleeping — the
        // refresh only fires when the file looks strictly newer.
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() + std::time::Duration::from_secs(60))
            .unwrap();
    }

    fn serve(dir: &Path) -> NanoWeb {
        let server = NanoWeb::new();
        server.populate_routes(dir, "").unwrap();
        server
    }

    fn refresh(server: &NanoWeb, dir: &Path, url: &str) {
        server
            .refresh_if_modified(as_requested(url), dir, "")
            .unwrap();
    }

    /// An index file is served under two URLs. Refreshing only the canonical
    /// one left `/` serving the bytes captured at startup — the single most
    /// common thing to edit in dev.
    #[test]
    fn editing_an_index_file_refreshes_its_directory_url() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("index.html"), "V1");
        write(&dir.join("sub/index.html"), "V1");

        let server = serve(dir);
        assert_eq!(body(&server, "/").as_deref(), Some("V1"));

        write(&dir.join("index.html"), "V2");
        write(&dir.join("sub/index.html"), "V2");
        refresh(&server, dir, "/");
        refresh(&server, dir, "/sub/");

        assert_eq!(body(&server, "/").as_deref(), Some("V2"));
        assert_eq!(body(&server, "/index.html").as_deref(), Some("V2"));
        assert_eq!(body(&server, "/sub/").as_deref(), Some("V2"));
        assert_eq!(body(&server, "/sub/index.html").as_deref(), Some("V2"));
    }

    #[test]
    fn files_created_after_startup_are_picked_up() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("index.html"), "V1");

        let server = serve(dir);
        assert_eq!(body(&server, "/added.html"), None);

        write(&dir.join("added.html"), "NEW");
        write(&dir.join("newdir/index.html"), "NEW-INDEX");
        refresh(&server, dir, "/added.html");
        refresh(&server, dir, "/newdir/");

        assert_eq!(body(&server, "/added.html").as_deref(), Some("NEW"));
        assert_eq!(body(&server, "/newdir/").as_deref(), Some("NEW-INDEX"));
    }

    #[test]
    fn deleted_files_stop_being_served() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("index.html"), "V1");
        write(&dir.join("doomed.html"), "DOOMED");

        let server = serve(dir);
        assert_eq!(body(&server, "/doomed.html").as_deref(), Some("DOOMED"));

        std::fs::remove_file(dir.join("doomed.html")).unwrap();
        refresh(&server, dir, "/doomed.html");

        assert_eq!(body(&server, "/doomed.html"), None);
    }

    #[test]
    fn deleting_an_index_file_clears_its_directory_url() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("index.html"), "V1");
        write(&dir.join("sub/index.html"), "SUB");

        let server = serve(dir);
        assert_eq!(body(&server, "/sub/").as_deref(), Some("SUB"));

        std::fs::remove_file(dir.join("sub/index.html")).unwrap();
        refresh(&server, dir, "/sub/");

        assert_eq!(body(&server, "/sub/"), None);
        assert_eq!(body(&server, "/sub/index.html"), None);
    }

    #[test]
    fn unchanged_files_are_not_rebuilt() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("index.html"), "V1");

        let server = serve(dir);
        assert!(!server.refresh_if_modified("/", dir, "").unwrap());
        assert!(!server.refresh_if_modified("/index.html", dir, "").unwrap());
    }
}
