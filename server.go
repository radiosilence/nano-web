package main

import (
	"bytes"
	"errors"
	"os"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog/log"
	"github.com/valyala/fasthttp"
)

var (
	// Pre-allocated byte slices for common strings
	healthPath       = []byte("/_health")
	contentTypeKey   = []byte("Content-Type")
	serverKey        = []byte("Server")
	serverValue      = []byte("nano-web")
	lastModifiedKey  = []byte("Last-Modified")
	eTagKey          = []byte("ETag")
	cacheControlKey  = []byte("Cache-Control")
	acceptEncoding   = []byte("Accept-Encoding")
	contentEncoding  = []byte("Content-Encoding")
	brEncoding       = []byte("br")
	gzipEncoding     = []byte("gzip")
	zstdEncoding     = []byte("zstd")

	// Metrics
	requestCount int64
	errorCount   int64
)

func healthCheckHandler(ctx *fasthttp.RequestCtx) {
	ctx.SetStatusCode(fasthttp.StatusOK)
	ctx.SetContentType("application/json")
	ctx.WriteString(`{"status":"ok","timestamp":"` + time.Now().UTC().Format(time.RFC3339) + `"}`)
}

func serveNotFound(ctx *fasthttp.RequestCtx) {
	atomic.AddInt64(&errorCount, 1)
	ctx.SetStatusCode(fasthttp.StatusNotFound)
	ctx.SetContentType("text/plain")
	ctx.WriteString("404 Not Found")
}

func refreshRouteIfModified(path string, route *Route) (*Route, error) {
	fileInfo, err := os.Stat(route.Path)
	log.Info().Str("path", route.Path).Msg("checking whether file has been modified")
	if err != nil {
		return nil, err
	}
	log.Info().Str("path", route.Path).Str("file modified", fileInfo.ModTime().String()).Str("route modified", route.ModTime.String()).Msg("comparing modtimes")
	if fileInfo.ModTime().After(route.ModTime) {
		content, err := os.ReadFile(route.Path)
		if err != nil {
			return nil, err
		}
		log.Debug().Str("path", route.Path).Msg("recaching route")
		route = makeRoute(route.Path, content, fileInfo.ModTime())
		routes.Lock()
		routes.m[path] = route
		routes.Unlock()
	} else {
		log.Debug().Str("path", route.Path).Msg("route not modified")
	}
	return route, nil
}

func resolveRoute(ctx *fasthttp.RequestCtx, s *ServeConfig, path string) (*Route, error) {
	// Get route
	route := getRoute(path)
	if route == nil {
		// Try with trailing slash for SPA mode
		if !bytes.HasSuffix(ctx.Path(), []byte("/")) {
			route = getRoute(path + "/")
		}

		// SPA fallback - serve index.html for unmatched routes
		if route == nil && s.SpaMode {
			route = getRoute("/")
		}

		if route == nil {
			return nil, errors.New("not found")
		}
	}
	return route, nil
}

func handler(ctx *fasthttp.RequestCtx, s *ServeConfig) {
	atomic.AddInt64(&requestCount, 1)

	path := string(ctx.Path())

	// Health check endpoints
	if bytes.Equal(ctx.Path(), healthPath) {
		healthCheckHandler(ctx)
		return
	}

	route, err := resolveRoute(ctx, s, path)

	if err != nil {
		serveNotFound(ctx)
		return
	}

	if s.Dev {
		refreshedRoute, err := refreshRouteIfModified(path, route)
		if err != nil {
			serveNotFound(ctx)
			return
		}
		route = refreshedRoute
	}

	// Set headers
	ctx.Response.Header.SetBytesKV(serverKey, serverValue)
	ctx.Response.Header.SetBytesKV(contentTypeKey, route.Headers.ContentType)
	ctx.Response.Header.SetBytesKV(eTagKey, route.Headers.ETag)
	ctx.Response.Header.SetBytesKV(lastModifiedKey, route.Headers.LastModified)
	ctx.Response.Header.SetBytesKV(cacheControlKey, route.Headers.CacheControl)

	// Handle compression
	acceptEncodingHeader := ctx.Request.Header.Peek("Accept-Encoding")
	if len(acceptEncodingHeader) > 0 && (route.Content.GzipLen > 0 || route.Content.BrotliLen > 0 || route.Content.ZstdLen > 0) {
		encoding := getAcceptedEncoding(acceptEncodingHeader)

		switch encoding {
		case "zstd":
			if route.Content.ZstdLen > 0 {
				ctx.Response.Header.SetBytesKV(contentEncoding, zstdEncoding)
				ctx.SetBody(route.Content.Zstd)
				return
			}
			fallthrough
		case "br":
			if route.Content.BrotliLen > 0 {
				ctx.Response.Header.SetBytesKV(contentEncoding, brEncoding)
				ctx.SetBody(route.Content.Brotli)
				return
			}
			fallthrough
		case "gzip":
			if route.Content.GzipLen > 0 {
				ctx.Response.Header.SetBytesKV(contentEncoding, gzipEncoding)
				ctx.SetBody(route.Content.Gzip)
				return
			}
		}
	}

	// Serve uncompressed content
	ctx.SetBody(route.Content.Plain)
}

func startServer(addr string, s *ServeConfig) error {
	server := &fasthttp.Server{
		Handler: func(ctx *fasthttp.RequestCtx) {
			start := time.Now()
			handler(ctx, s)

			if s.LogRequests {
				duration := time.Since(start)
				log.Info().
					Str("method", string(ctx.Method())).
					Str("path", string(ctx.Path())).
					Int("status", ctx.Response.StatusCode()).
					Dur("duration", duration).
					Int("bytes", len(ctx.Response.Body())).
					Msg("request handled")
			}
		},
		Name:               "nano-web",
		ReadTimeout:        10 * time.Second,
		WriteTimeout:       10 * time.Second,
		MaxRequestBodySize: 1 << 20, // 1MB
	}

	log.Info().
		Str("addr", addr).
		Bool("log_requests", s.LogRequests).
		Msg("starting server")

	return server.ListenAndServe(addr)
}
