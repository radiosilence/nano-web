Released 2025-05-27.

### Added

- Development mode with automatic file change detection and reloading
- Real-time file modification monitoring

### Changed

- Enhanced version output with fire emoji

### Technical

- Added Dev flag to ServeCmd struct
- Implemented file modification time checking in dev mode
- Added automatic route recaching when files are modified
- Enhanced route structure with Path, ModTime, and Headers fields
- Improved ETag generation using MD5 hashing of modification timestamps
- Added comprehensive file stat checking and error handling
