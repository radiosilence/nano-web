package main

import (
	"bytes"
	"compress/gzip"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/andybalholm/brotli"
	"github.com/valyala/fasthttp"
)

func TestGetAppEnv(t *testing.T) {
	// Set test environment variables
	os.Setenv("VITE_API_URL", "https://api.example.com")
	os.Setenv("VITE_DEBUG", "true")
	os.Setenv("OTHER_VAR", "should_not_be_included")
	defer func() {
		os.Unsetenv("VITE_API_URL")
		os.Unsetenv("VITE_DEBUG")
		os.Unsetenv("OTHER_VAR")
	}()

	result := getAppEnv("VITE_")

	// Check that VITE_ prefixed variables are included
	if result["API_URL"] != "https://api.example.com" {
		t.Errorf("Expected API_URL to be 'https://api.example.com', got '%s'", result["API_URL"])
	}

	if result["DEBUG"] != "true" {
		t.Errorf("Expected DEBUG to be 'true', got '%s'", result["DEBUG"])
	}

	// Check that non-VITE_ prefixed variables are not included
	if _, exists := result["OTHER_VAR"]; exists {
		t.Errorf("Expected OTHER_VAR to not be included in result")
	}
}

func TestGetMimetype(t *testing.T) {
	tests := []struct {
		path     string
		expected []byte
	}{
		{"test.html", []byte("text/html")},
		{"test.css", []byte("text/css")},
		{"test.js", []byte("text/javascript")},
		{"test.json", []byte("application/json")},
		{"test.png", []byte("image/png")},
		{"test.jpg", []byte("image/jpeg")},
		{"test.jpeg", []byte("image/jpeg")},
		{"test.gif", []byte("image/gif")},
		{"test.svg", []byte("image/svg+xml")},
		{"test.ico", []byte("image/x-icon")},
		{"test.webp", []byte("image/webp")},
		{"test.pdf", []byte("application/pdf")},
		{"test.zip", []byte("application/zip")},
		{"test.mp4", []byte("video/mp4")},
		{"test.mp3", []byte("audio/mpeg")},
		{"test.wav", []byte("audio/wav")},
		{"test.ogg", []byte("audio/ogg")},
		{"test.txt", []byte("text/plain")},
		{"test.csv", []byte("text/csv")},
		{"test.unknown", []byte("application/octet-stream")},
		{"test", []byte("application/octet-stream")},
	}

	for _, tt := range tests {
		t.Run(tt.path, func(t *testing.T) {
			result := getMimetype(tt.path)
			if !bytes.Equal(result, tt.expected) {
				t.Errorf("getMimetype(%s) = %s, want %s", tt.path, result, tt.expected)
			}
		})
	}
}

func TestShouldTemplate(t *testing.T) {
	tests := []struct {
		mimetype []byte
		expected bool
	}{
		{[]byte("text/html"), true},
		{[]byte("text/css"), false},
		{[]byte("text/javascript"), false},
		{[]byte("application/json"), false},
		{[]byte("image/png"), false},
		{[]byte("application/pdf"), false},
		{[]byte("video/mp4"), false},
	}

	for _, tt := range tests {
		t.Run(string(tt.mimetype), func(t *testing.T) {
			result := shouldTemplate(tt.mimetype)
			if result != tt.expected {
				t.Errorf("shouldTemplate(%s) = %v, want %v", tt.mimetype, result, tt.expected)
			}
		})
	}
}

func TestShouldCompress(t *testing.T) {
	tests := []struct {
		mimetype []byte
		expected bool
	}{
		{[]byte("text/html"), true},
		{[]byte("text/css"), true},
		{[]byte("text/javascript"), true},
		{[]byte("application/json"), true},
		{[]byte("image/png"), false},
		{[]byte("application/pdf"), false},
		{[]byte("video/mp4"), false},
	}

	for _, tt := range tests {
		t.Run(string(tt.mimetype), func(t *testing.T) {
			result := shouldCompress(tt.mimetype)
			if result != tt.expected {
				t.Errorf("shouldCompress(%s) = %v, want %v", tt.mimetype, result, tt.expected)
			}
		})
	}
}

func TestGzipData(t *testing.T) {
	testData := []byte("Hello, World! This is test data for compression.")
	compressed := gzipData(testData)

	if len(compressed) == 0 {
		t.Error("gzipData() returned empty data")
	}

	// Verify it's actually compressed data by trying to decompress it
	reader, err := gzip.NewReader(bytes.NewReader(compressed))
	if err != nil {
		t.Fatalf("Failed to create gzip reader: %v", err)
	}
	defer reader.Close()

	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress data: %v", err)
	}

	if !bytes.Equal(decompressed, testData) {
		t.Errorf("Decompressed data doesn't match original. Got %s, want %s", decompressed, testData)
	}
}

func TestBrotliData(t *testing.T) {
	testData := []byte("Hello, World! This is test data for compression.")
	compressed := brotliData(testData)

	if len(compressed) == 0 {
		t.Error("brotliData() returned empty data")
	}

	// Verify it's actually compressed data by trying to decompress it
	reader := brotli.NewReader(bytes.NewReader(compressed))
	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress brotli data: %v", err)
	}

	if !bytes.Equal(decompressed, testData) {
		t.Errorf("Decompressed data doesn't match original. Got %s, want %s", decompressed, testData)
	}
}

func TestTemplateRoute(t *testing.T) {
	// Set up test environment
	appEnv = map[string]string{
		"API_URL": "https://api.example.com",
		"DEBUG":   "true",
	}

	content := []byte(`<html><body>API: {{.Env.API_URL}}, Debug: {{.Env.DEBUG}}</body></html>`)
	result, err := templateRoute("test.html", content)

	if err != nil {
		t.Fatalf("templateRoute() error = %v", err)
	}

	resultStr := string(result)
	if !strings.Contains(resultStr, "https://api.example.com") {
		t.Errorf("templateRoute() result doesn't contain expected API_URL")
	}

	if !strings.Contains(resultStr, "true") {
		t.Errorf("templateRoute() result doesn't contain expected DEBUG value")
	}
}

func TestGetAcceptedEncoding(t *testing.T) {
	tests := []struct {
		name           string
		acceptEncoding string
		expected       int
	}{
		{"brotli preferred", "gzip, deflate, br", 2},
		{"gzip only", "gzip, deflate", 1},
		{"no compression", "identity", 0},
		{"empty header", "", 0},
		{"brotli first", "br, gzip", 2},
		{"gzip first", "gzip, br", 2}, // br is still preferred
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := &fasthttp.RequestCtx{}
			ctx.Request.Header.Set("Accept-Encoding", tt.acceptEncoding)

			result := getAcceptedEncoding(ctx)
			if result != tt.expected {
				t.Errorf("getAcceptedEncoding() = %v, want %v", result, tt.expected)
			}
		})
	}
}

func setupTestFiles(t *testing.T) string {
	tmpDir := t.TempDir()

	// Create test files
	files := map[string]string{
		"index.html":     "<html><body>{{.Env.API_URL}}</body></html>",
		"style.css":      "body { margin: 0; }",
		"script.js":      "console.log('{{.Env.DEBUG}}');",
		"data.json":      `{"key": "{{.Env.API_URL}}"}`,
		"image.png":      "fake png data",
		"document.pdf":   "fake pdf data",
		"nested/page.html": "<html><body>Nested</body></html>",
	}

	for path, content := range files {
		fullPath := filepath.Join(tmpDir, path)
		dir := filepath.Dir(fullPath)
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatalf("Failed to create directory %s: %v", dir, err)
		}
		if err := os.WriteFile(fullPath, []byte(content), 0644); err != nil {
			t.Fatalf("Failed to create test file %s: %v", fullPath, err)
		}
	}

	return tmpDir
}

func TestMakeRoute(t *testing.T) {
	tmpDir := setupTestFiles(t)

	// Set up test environment for templating
	appEnv = map[string]string{
		"API_URL": "https://api.example.com",
		"DEBUG":   "true",
	}

	tests := []struct {
		name        string
		path        string
		expectError bool
		contentType []byte
		checkGzip   bool
		checkBrotli bool
	}{
		{
			name:        "HTML file with templating",
			path:        "index.html",
			expectError: false,
			contentType: []byte("text/html"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "CSS file with templating",
			path:        "style.css",
			expectError: false,
			contentType: []byte("text/css"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "JavaScript file with templating",
			path:        "script.js",
			expectError: false,
			contentType: []byte("text/javascript"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "JSON file with templating",
			path:        "data.json",
			expectError: false,
			contentType: []byte("application/json"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "PNG file without compression",
			path:        "image.png",
			expectError: false,
			contentType: []byte("image/png"),
			checkGzip:   false,
			checkBrotli: false,
		},
		{
			name:        "PDF file without compression",
			path:        "document.pdf",
			expectError: false,
			contentType: []byte("application/pdf"),
			checkGzip:   false,
			checkBrotli: false,
		},
		{
			name:        "Non-existent file",
			path:        "nonexistent.txt",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fullPath := filepath.Join(tmpDir, tt.path)
			route, err := makeRoute(fullPath)

			if tt.expectError {
				if err == nil {
					t.Errorf("makeRoute() expected error but got none")
				}
				return
			}

			if err != nil {
				t.Fatalf("makeRoute() error = %v", err)
			}

			if !bytes.Equal(route.ContentType, tt.contentType) {
				t.Errorf("makeRoute() ContentType = %s, want %s", route.ContentType, tt.contentType)
			}

			if tt.checkGzip {
				if len(route.Content.Gzip) == 0 {
					t.Errorf("makeRoute() expected gzip compression but got none")
				}
				if route.Content.GzipLen != len(route.Content.Gzip) {
					t.Errorf("makeRoute() GzipLen = %d, want %d", route.Content.GzipLen, len(route.Content.Gzip))
				}
			} else {
				if len(route.Content.Gzip) != 0 {
					t.Errorf("makeRoute() expected no gzip compression but got some")
				}
			}

			if tt.checkBrotli {
				if len(route.Content.Brotli) == 0 {
					t.Errorf("makeRoute() expected brotli compression but got none")
				}
				if route.Content.BrotliLen != len(route.Content.Brotli) {
					t.Errorf("makeRoute() BrotliLen = %d, want %d", route.Content.BrotliLen, len(route.Content.Brotli))
				}
			} else {
				if len(route.Content.Brotli) != 0 {
					t.Errorf("makeRoute() expected no brotli compression but got some")
				}
			}

			if route.Content.PlainLen != len(route.Content.Plain) {
				t.Errorf("makeRoute() PlainLen = %d, want %d", route.Content.PlainLen, len(route.Content.Plain))
			}
		})
	}
}

func TestHandler(t *testing.T) {
	tmpDir := setupTestFiles(t)

	// Set up test environment
	appEnv = map[string]string{
		"API_URL": "https://api.example.com",
		"DEBUG":   "true",
	}

	// Initialize routes
	routes = &Routes{
		m: make(map[string]*Route),
	}

	// Populate routes from test directory
	populateRoutes(tmpDir)

	// Reset counters
	atomic.StoreUint64(&requestCount, 0)
	atomic.StoreUint64(&errorCount, 0)

	tests := []struct {
		name           string
		path           string
		method         string
		acceptEncoding string
		spaMode        bool
		expectedStatus int
		expectedType   []byte
	}{
		{
			name:           "serve index.html",
			path:           "/",
			method:         "GET",
			acceptEncoding: "",
			spaMode:        false,
			expectedStatus: 200,
			expectedType:   []byte("text/html"),
		},
		{
			name:           "serve CSS with gzip",
			path:           "/style.css",
			method:         "GET",
			acceptEncoding: "gzip",
			spaMode:        false,
			expectedStatus: 200,
			expectedType:   []byte("text/css"),
		},
		{
			name:           "serve JS with brotli",
			path:           "/script.js",
			method:         "GET",
			acceptEncoding: "br, gzip",
			spaMode:        false,
			expectedStatus: 200,
			expectedType:   []byte("text/javascript"),
		},
		{
			name:           "404 without SPA mode",
			path:           "/nonexistent",
			method:         "GET",
			acceptEncoding: "",
			spaMode:        false,
			expectedStatus: 404,
		},
		{
			name:           "404 with SPA mode falls back to index",
			path:           "/nonexistent",
			method:         "GET",
			acceptEncoding: "",
			spaMode:        true,
			expectedStatus: 200,
			expectedType:   []byte("text/html"),
		},
		{
			name:           "health check",
			path:           "/health",
			method:         "GET",
			acceptEncoding: "",
			spaMode:        false,
			expectedStatus: 200,
			expectedType:   []byte("application/json"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := &fasthttp.RequestCtx{}
			ctx.Request.SetRequestURI(tt.path)
			ctx.Request.Header.SetMethod(tt.method)
			if tt.acceptEncoding != "" {
				ctx.Request.Header.Set("Accept-Encoding", tt.acceptEncoding)
			}

			// Create a ServeCmd with test configuration
			serveCmd := &ServeCmd{
				PublicDir:    tmpDir,
				SpaMode:      tt.spaMode,
				LogRequests:  false, // Disable for tests
				LogLevel:     "error",
				LogFormat:    "json",
				ConfigPrefix: "VITE_",
			}

			handler(ctx, serveCmd)

			if ctx.Response.StatusCode() != tt.expectedStatus {
				t.Errorf("handler() status = %d, want %d", ctx.Response.StatusCode(), tt.expectedStatus)
			}

			if tt.expectedType != nil {
				contentType := ctx.Response.Header.Peek("Content-Type")
				if !bytes.Equal(contentType, tt.expectedType) {
					t.Errorf("handler() Content-Type = %s, want %s", contentType, tt.expectedType)
				}
			}
		})
	}
}

func TestPopulateRoutes(t *testing.T) {
	tmpDir := setupTestFiles(t)

	// Initialize routes
	routes = &Routes{
		m: make(map[string]*Route),
	}

	populateRoutes(tmpDir)

	// Check that routes were created
	expectedRoutes := []string{
		"/index.html",
		"/",          // index route
		"//",         // index route with trailing slash
		"/style.css",
		"/script.js",
		"/data.json",
		"/image.png",
		"/document.pdf",
		"/nested/page.html",
	}

	for _, routePath := range expectedRoutes {
		if _, exists := getRoute(routePath); !exists {
			t.Errorf("Expected route %s was not created", routePath)
		}
	}

	// Verify route count
	routes.RLock()
	routeCount := len(routes.m)
	routes.RUnlock()

	if routeCount == 0 {
		t.Error("No routes were populated")
	}
}

func BenchmarkHandler(b *testing.B) {
	tmpDir := setupTestFiles(&testing.T{})

	// Set up test environment
	appEnv = map[string]string{
		"API_URL": "https://api.example.com",
		"DEBUG":   "true",
	}

	// Initialize routes
	routes = &Routes{
		m: make(map[string]*Route),
	}
	populateRoutes(tmpDir)

	serveCmd := &ServeCmd{
		PublicDir:    tmpDir,
		SpaMode:      false,
		LogRequests:  false,
		LogLevel:     "error",
		LogFormat:    "json",
		ConfigPrefix: "VITE_",
	}

	ctx := &fasthttp.RequestCtx{}
	ctx.Request.SetRequestURI("/")
	ctx.Request.Header.SetMethod("GET")

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		ctx.Response.Reset()
		handler(ctx, serveCmd)
	}
}

func BenchmarkGzipCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is test data for compression. ", 100))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		gzipData(data)
	}
}

func BenchmarkBrotliCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is test data for compression. ", 100))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		brotliData(data)
	}
}