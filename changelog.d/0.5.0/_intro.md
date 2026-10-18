Released 2025-05-27.

### Changed

- Major code restructuring and refactoring
- Moved source files to dedicated src/ directory structure

### Fixed

- Corrected default port configuration
- Resolved refactoring-related issues

### Technical

- Split monolithic main.go into modular components: compression.go, routes.go, server.go, template.go, mimetypes.go
- Enhanced MIME type handling with comprehensive type definitions
- Improved compression logic separation
- Better code organization and maintainability
- Updated Dockerfile and Taskfile.yml for new structure
