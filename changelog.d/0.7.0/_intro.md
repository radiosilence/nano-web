Released 2025-05-30.

### Added

- Aggressive HTTP cache headers with intelligent cache control
- Asset-specific caching strategies (1 year for assets, 15 minutes for HTML, 1 hour for other content)

### Technical

- Added CacheControl header to Routes struct
- Implemented getCacheControl function with MIME type-based caching rules
- Enhanced isAsset detection for CSS, JavaScript, images, fonts, audio, and video
- Removed benchmark pipeline and updated CI configuration
- Improved README with badges and cleaner documentation
