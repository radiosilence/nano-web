package main

import "strings"

// MIME type mappings for file extensions
var mimetypes = map[string][]byte{
	// Text/code files
	".bash":          []byte("text/plain"),
	".c":             []byte("text/plain"),
	".cc":            []byte("text/plain"),
	".cfg":           []byte("text/plain"),
	".clj":           []byte("text/plain"),
	".cljs":          []byte("text/plain"),
	".cmake":         []byte("text/plain"),
	".conf":          []byte("text/plain"),
	".cpp":           []byte("text/plain"),
	".cr":            []byte("text/plain"),
	".cs":            []byte("text/plain"),
	".css":           []byte("text/css"),
	".csv":           []byte("text/csv"),
	".cxx":           []byte("text/plain"),
	".dart":          []byte("text/plain"),
	".dockerfile":    []byte("text/plain"),
	".editorconfig":  []byte("text/plain"),
	".edn":           []byte("text/plain"),
	".elm":           []byte("text/plain"),
	".env":           []byte("text/plain"),
	".eslintrc":      []byte("text/plain"),
	".ex":            []byte("text/plain"),
	".exs":           []byte("text/plain"),
	".fish":          []byte("text/plain"),
	".fs":            []byte("text/plain"),
	".gitattributes": []byte("text/plain"),
	".gitignore":     []byte("text/plain"),
	".go":            []byte("text/plain"),
	".gradle":        []byte("text/plain"),
	".h":             []byte("text/plain"),
	".hpp":           []byte("text/plain"),
	".hs":            []byte("text/plain"),
	".html":          []byte("text/html"),
	".htm":           []byte("text/html"),
	".ini":           []byte("text/plain"),
	".java":          []byte("text/plain"),
	".jl":            []byte("text/plain"),
	".js":            []byte("text/javascript"),
	".json":          []byte("application/json"),
	".jsx":           []byte("text/javascript"),
	".kt":            []byte("text/plain"),
	".lock":          []byte("text/plain"),
	".log":           []byte("text/plain"),
	".lua":           []byte("text/plain"),
	".makefile":      []byte("text/plain"),
	".markdown":      []byte("text/markdown"),
	".md":            []byte("text/markdown"),
	".ml":            []byte("text/plain"),
	".nim":           []byte("text/plain"),
	".php":           []byte("text/plain"),
	".po":            []byte("text/plain"),
	".pom":           []byte("text/xml"),
	".prettierrc":    []byte("text/plain"),
	".ps1":           []byte("text/plain"),
	".py":            []byte("text/plain"),
	".r":             []byte("text/plain"),
	".rb":            []byte("text/plain"),
	".rs":            []byte("text/plain"),
	".sbt":           []byte("text/plain"),
	".scala":         []byte("text/plain"),
	".scss":          []byte("text/css"),
	".sass":          []byte("text/css"),
	".less":          []byte("text/css"),
	".sh":            []byte("text/plain"),
	".sql":           []byte("text/plain"),
	".svelte":        []byte("text/plain"),
	".swift":         []byte("text/plain"),
	".toml":          []byte("text/plain"),
	".ts":            []byte("text/plain"),
	".tsx":           []byte("text/plain"),
	".txt":           []byte("text/plain"),
	".v":             []byte("text/plain"),
	".vue":           []byte("text/plain"),
	".xml":           []byte("application/xml"),
	".xsl":           []byte("application/xml"),
	".xslt":          []byte("application/xml"),
	".yaml":          []byte("text/plain"),
	".yml":           []byte("text/plain"),
	".zig":           []byte("text/plain"),
	".zsh":           []byte("text/plain"),

	// Images
	".gif":  []byte("image/gif"),
	".ico":  []byte("image/x-icon"),
	".jpeg": []byte("image/jpeg"),
	".jpg":  []byte("image/jpeg"),
	".png":  []byte("image/png"),
	".bmp":  []byte("image/bmp"),
	".svg":  []byte("image/svg+xml"),
	".webp": []byte("image/webp"),
	".tiff": []byte("image/tiff"),
	".tif":  []byte("image/tiff"),
	".avif": []byte("image/avif"),
	".heic": []byte("image/heic"),
	".heif": []byte("image/heif"),

	// Fonts
	".eot":   []byte("application/vnd.ms-fontobject"),
	".otf":   []byte("font/otf"),
	".ttf":   []byte("font/ttf"),
	".woff":  []byte("font/woff"),
	".woff2": []byte("font/woff2"),

	// Audio
	".mp3":  []byte("audio/mpeg"),
	".ogg":  []byte("audio/ogg"),
	".wav":  []byte("audio/wav"),
	".aac":  []byte("audio/aac"),
	".flac": []byte("audio/flac"),
	".m4a":  []byte("audio/mp4"),
	".opus": []byte("audio/opus"),

	// Video
	".mp4":  []byte("video/mp4"),
	".webm": []byte("video/webm"),
	".avi":  []byte("video/x-msvideo"),
	".mov":  []byte("video/quicktime"),
	".wmv":  []byte("video/x-ms-wmv"),
	".flv":  []byte("video/x-flv"),
	".mkv":  []byte("video/x-matroska"),
	".m4v":  []byte("video/mp4"),

	// Documents
	".doc":  []byte("application/msword"),
	".docx": []byte("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
	".xls":  []byte("application/vnd.ms-excel"),
	".xlsx": []byte("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
	".ppt":  []byte("application/vnd.ms-powerpoint"),
	".pptx": []byte("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
	".pdf":  []byte("application/pdf"),
	".rtf":  []byte("application/rtf"),
	".odt":  []byte("application/vnd.oasis.opendocument.text"),
	".ods":  []byte("application/vnd.oasis.opendocument.spreadsheet"),
	".odp":  []byte("application/vnd.oasis.opendocument.presentation"),

	// Archives
	".zip": []byte("application/zip"),
	".tar": []byte("application/x-tar"),
	".gz":  []byte("application/gzip"),
	".bz2": []byte("application/x-bzip2"),
	".rar": []byte("application/vnd.rar"),
	".7z":  []byte("application/x-7z-compressed"),
	".xz":  []byte("application/x-xz"),

	// Other common types
	".jsonld":      []byte("application/ld+json"),
	".rss":         []byte("application/rss+xml"),
	".atom":        []byte("application/atom+xml"),
	".manifest":    []byte("application/manifest+json"),
	".webmanifest": []byte("application/manifest+json"),
	".appcache":    []byte("text/cache-manifest"),
	".map":         []byte("application/json"),
	".bin":         []byte("application/octet-stream"),
	".exe":         []byte("application/octet-stream"),
	".dmg":         []byte("application/octet-stream"),
	".deb":         []byte("application/octet-stream"),
	".rpm":         []byte("application/octet-stream"),
	".msi":         []byte("application/octet-stream"),
}

var defaultMimetype = []byte("application/octet-stream")

// MIME types that should be templated
var templateableMimeTypes = map[string]bool{
	"text/html": true,
}

// MIME types that should be compressed
var compressibleMimeTypes = map[string]bool{
	// Text-based formats
	"text/html":                 true,
	"text/css":                  true,
	"text/javascript":           true,
	"text/plain":                true,
	"text/csv":                  true,
	"text/markdown":             true,
	"text/cache-manifest":       true,
	"application/json":          true,
	"application/ld+json":       true,
	"application/manifest+json": true,
	"text/xml":                  true,
	"application/xml":           true,
	"application/rss+xml":       true,
	"application/atom+xml":      true,
	"image/svg+xml":             true,
}

// MIME types that should NOT be compressed (binary/already compressed)
var nonCompressibleMimeTypes = map[string]bool{
	// Images (already compressed)
	"image/jpeg":   true,
	"image/png":    true,
	"image/gif":    true,
	"image/webp":   true,
	"image/avif":   true,
	"image/heic":   true,
	"image/heif":   true,
	"image/bmp":    true,
	"image/tiff":   true,
	"image/x-icon": true,

	// Audio (already compressed)
	"audio/mpeg": true,
	"audio/mp4":  true,
	"audio/aac":  true,
	"audio/ogg":  true,
	"audio/flac": true,
	"audio/opus": true,
	"audio/wav":  true,

	// Video (already compressed)
	"video/mp4":        true,
	"video/webm":       true,
	"video/x-msvideo":  true,
	"video/quicktime":  true,
	"video/x-ms-wmv":   true,
	"video/x-flv":      true,
	"video/x-matroska": true,

	// Fonts (binary/already optimized)
	"font/woff":                     true,
	"font/woff2":                    true,
	"font/ttf":                      true,
	"font/otf":                      true,
	"application/vnd.ms-fontobject": true,

	// Archives (already compressed)
	"application/zip":             true,
	"application/gzip":            true,
	"application/x-bzip2":         true,
	"application/vnd.rar":         true,
	"application/x-7z-compressed": true,
	"application/x-xz":            true,
	"application/x-tar":           true,

	// Documents (binary/already optimized)
	"application/pdf":    true,
	"application/msword": true,
	"application/vnd.openxmlformats-officedocument.wordprocessingml.document": true,
	"application/vnd.ms-excel": true,
	"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet":         true,
	"application/vnd.ms-powerpoint":                                             true,
	"application/vnd.openxmlformats-officedocument.presentationml.presentation": true,
	"application/vnd.oasis.opendocument.text":                                   true,
	"application/vnd.oasis.opendocument.spreadsheet":                            true,
	"application/vnd.oasis.opendocument.presentation":                           true,

	// Other binary formats
	"application/octet-stream": true,
}

func getMimetype(path string) []byte {
	if idx := strings.LastIndexByte(path, '.'); idx > 0 {
		ext := strings.ToLower(path[idx:])
		if mimetype, ok := mimetypes[ext]; ok {
			return mimetype
		}
	}
	return defaultMimetype
}

func shouldTemplate(mimetype []byte) bool {
	return templateableMimeTypes[string(mimetype)]
}

func shouldCompress(mimetype []byte) bool {
	mimeStr := string(mimetype)

	// Check if explicitly marked as non-compressible
	if nonCompressibleMimeTypes[mimeStr] {
		return false
	}

	// Check if explicitly marked as compressible
	return compressibleMimeTypes[mimeStr]
}
