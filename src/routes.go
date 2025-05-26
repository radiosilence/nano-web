package main

import (
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/rs/zerolog/log"
)

type Route struct {
	Content      Content
	ContentType  []byte
	LastModified []byte
}

type Content struct {
	Plain     []byte
	Gzip      []byte
	Brotli    []byte
	PlainLen  int
	GzipLen   int
	BrotliLen int
}

type Routes struct {
	sync.RWMutex
	m map[string]*Route
}

var routes *Routes

func makeRoute(path string, content []byte) *Route {
	mimetype := getMimetype(path)

	// Check if file should be templated
	if shouldTemplate(mimetype) {
		templated, err := templateRoute(path, content)
		if err != nil {
			log.Error().Err(err).Str("path", path).Msg("error templating file")
			// Fall back to original content if templating fails
		} else {
			content = templated
		}
	}

	route := &Route{
		Content: Content{
			Plain:    content,
			PlainLen: len(content),
		},
		ContentType:  mimetype,
		LastModified: []byte("Mon, 01 Jan 2024 00:00:00 GMT"), // Static last modified
	}

	// Compress if appropriate
	if shouldCompress(mimetype) {
		route.Content.Gzip = gzipData(content)
		route.Content.GzipLen = len(route.Content.Gzip)
		route.Content.Brotli = brotliData(content)
		route.Content.BrotliLen = len(route.Content.Brotli)
	}

	return route
}

func populateRoutes(publicDir string) error {
	routes = &Routes{m: make(map[string]*Route)}

	log.Debug().Str("public_dir", publicDir).Msg("starting route population")

	err := filepath.Walk(publicDir, func(filePath string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		if info.IsDir() {
			return nil
		}

		// Replace the public directory path with empty string to get the URL path
		urlPath := strings.Replace(filePath, publicDir, "", 1)
		log.Debug().
			Str("public_dir", publicDir).
			Str("original_path", filePath).
			Str("replaced_path", urlPath).
			Msg("processing file path")

		// Ensure the URL path starts with /
		if !strings.HasPrefix(urlPath, "/") {
			urlPath = "/" + urlPath
		}

		// Debug logging for file names and paths
		log.Debug().
			Str("file_name", info.Name()).
			Str("file_path", filePath).
			Msg("debug file name")

		log.Debug().
			Str("url_path", urlPath).
			Msg("debug url path")

		// Read file content
		content, err := os.ReadFile(filePath)
		if err != nil {
			log.Error().Err(err).Str("path", filePath).Msg("error reading file")
			return nil // Continue processing other files
		}

		// Create route
		route := makeRoute(filePath, content)

		routes.Lock()
		routes.m[urlPath] = route

		// Handle index files
		if info.Name() == "index.html" {
			// Extract directory path
			dir := filepath.Dir(urlPath)
			log.Debug().Str("index_url_path", dir).Msg("index url path")

			// Normalize directory path
			if dir == "." || dir == "/" {
				dir = "/"
			} else if !strings.HasSuffix(dir, "/") {
				dir += "/"
			}

			log.Debug().
				Str("index_path", dir).
				Str("file_path", filePath).
				Msg("adding index route")

			routes.m[dir] = route

			// Also add with double slash for root
			if dir == "/" {
				routes.m["//"] = route
			}
		}
		routes.Unlock()

		return nil
	})

	if err != nil {
		return err
	}

	routes.RLock()
	routeCount := len(routes.m)
	routes.RUnlock()

	// Debug: print all registered routes
	routes.RLock()
	for path := range routes.m {
		log.Debug().Str("route", path).Msg("registered route")
	}
	routes.RUnlock()

	log.Info().Int("route_count", routeCount).Msg("routes populated successfully")
	return nil
}

func getRoute(path string) *Route {
	log.Debug().Str("searching_path", path).Msg("searching for route")

	routes.RLock()
	defer routes.RUnlock()

	if route, exists := routes.m[path]; exists {
		return route
	}
	return nil
}