package main

import (
	"bytes"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog/log"
	"github.com/valyala/fasthttp"
)

var (
	// Pre-allocated byte slices for common strings
	healthPath      = []byte("/health")
	altHealthPath   = []byte("/_health")
	contentTypeKey  = []byte("Content-Type")
	serverKey       = []byte("Server")
	serverValue     = []byte("nano-web")
	lastModifiedKey = []byte("Last-Modified")
	acceptEncoding  = []byte("Accept-Encoding")
	contentEncoding = []byte("Content-Encoding")
	brEncoding      = []byte("br")
	gzipEncoding    = []byte("gzip")

	// Metrics
	requestCount int64
	errorCount   int64
)

func healthCheckHandler(ctx *fasthttp.RequestCtx) {
	ctx.SetStatusCode(fasthttp.StatusOK)
	ctx.SetContentType("application/json")
	ctx.WriteString(`{"status":"ok","timestamp":"` + time.Now().UTC().Format(time.RFC3339) + `"}`)
}

func handler(ctx *fasthttp.RequestCtx) {
	atomic.AddInt64(&requestCount, 1)

	path := string(ctx.Path())

	// Health check endpoints
	if bytes.Equal(ctx.Path(), healthPath) || bytes.Equal(ctx.Path(), altHealthPath) {
		healthCheckHandler(ctx)
		return
	}

	// Get route
	route := getRoute(path)
	if route == nil {
		// Try with trailing slash for SPA mode
		if !bytes.HasSuffix(ctx.Path(), []byte("/")) {
			route = getRoute(path + "/")
		}
		
		// SPA fallback - serve index.html for unmatched routes
		if route == nil {
			route = getRoute("/")
		}
		
		if route == nil {
			atomic.AddInt64(&errorCount, 1)
			ctx.SetStatusCode(fasthttp.StatusNotFound)
			ctx.SetContentType("text/plain")
			ctx.WriteString("404 Not Found")
			return
		}
	}

	// Set headers
	ctx.Response.Header.SetBytesKV(contentTypeKey, route.ContentType)
	ctx.Response.Header.SetBytesKV(serverKey, serverValue)
	ctx.Response.Header.SetBytesKV(lastModifiedKey, route.LastModified)

	// Handle compression
	acceptEncodingHeader := ctx.Request.Header.Peek("Accept-Encoding")
	if len(acceptEncodingHeader) > 0 && (route.Content.GzipLen > 0 || route.Content.BrotliLen > 0) {
		encoding := getAcceptedEncoding(acceptEncodingHeader)
		
		switch encoding {
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

func startServer(addr string, logRequests bool) error {
	server := &fasthttp.Server{
		Handler: func(ctx *fasthttp.RequestCtx) {
			start := time.Now()
			handler(ctx)
			
			if logRequests {
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
		Bool("log_requests", logRequests).
		Msg("starting server")

	return server.ListenAndServe(addr)
}