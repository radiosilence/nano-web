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

func TestGetEnv(t *testing.T) {
	tests := []struct {
		name     string
		envKey   string
		envValue string
		fallback string
		expected string
	}{
		{
			name:     "environment variable exists",
			envKey:   "TEST_VAR",
			envValue: "test_value",
			fallback: "fallback",
			expected: "test_value",
		},
		{
			name:     "environment variable does not exist",
			envKey:   "NON_EXISTENT_VAR",
			envValue: "",
			fallback: "fallback",
			expected: "fallback",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.envValue != "" {
				os.Setenv(tt.envKey, tt.envValue)
				defer os.Unsetenv(tt.envKey)
			}

			result := getEnv(tt.envKey, tt.fallback)
			if result != tt.expected {
				t.Errorf("getEnv() = %v, want %v", result, tt.expected)
			}
		})
	}
}

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

	result := getAppEnv()

	expected := map[string]string{
		"API_URL": "https://api.example.com",
		"DEBUG":   "true",
	}

	if len(result) < 2 {
		t.Errorf("getAppEnv() returned fewer variables than expected")
	}

	for key, expectedValue := range expected {
		if value, exists := result[key]; !exists {
			t.Errorf("getAppEnv() missing key %s", key)
		} else if value != expectedValue {
			t.Errorf("getAppEnv()[%s] = %v, want %v", key, value, expectedValue)
		}
	}

	if _, exists := result["OTHER_VAR"]; exists {
		t.Errorf("getAppEnv() should not include OTHER_VAR")
	}
}

func TestGetMimetype(t *testing.T) {
	tests := []struct {
		path     string
		expected []byte
	}{
		{"file.html", []byte("text/html")},
		{"style.css", []byte("text/css")},
		{"script.js", []byte("text/javascript")},
		{"data.json", []byte("application/json")},
		{"image.png", []byte("image/png")},
		{"photo.jpg", []byte("image/jpeg")},
		{"photo.jpeg", []byte("image/jpeg")},
		{"animation.gif", []byte("image/gif")},
		{"icon.svg", []byte("image/svg+xml")},
		{"favicon.ico", []byte("image/x-icon")},
		{"modern.webp", []byte("image/webp")},
		{"document.pdf", []byte("application/pdf")},
		{"archive.zip", []byte("application/zip")},
		{"video.mp4", []byte("video/mp4")},
		{"video.webm", []byte("video/webm")},
		{"audio.mp3", []byte("audio/mpeg")},
		{"sound.wav", []byte("audio/wav")},
		{"music.ogg", []byte("audio/ogg")},
		{"text.txt", []byte("text/plain")},
		{"data.csv", []byte("text/csv")},
		{"config.xml", []byte("application/xml")},
		{"font.ttf", []byte("font/ttf")},
		{"font.woff", []byte("font/woff")},
		{"font.woff2", []byte("font/woff2")},
		{"unknown.xyz", defaultMimetype},
		{"noextension", defaultMimetype},
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
		{[]byte("text/css"), true},
		{[]byte("text/javascript"), true},
		{[]byte("application/json"), true},
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
	input := []byte("Hello, World! This is a test string for compression.")
	compressed := gzipData(input)

	// Verify it's actually compressed (should be different)
	if bytes.Equal(input, compressed) {
		t.Error("gzipData() did not compress the data")
	}

	// Verify we can decompress it back
	reader, err := gzip.NewReader(bytes.NewReader(compressed))
	if err != nil {
		t.Fatalf("Failed to create gzip reader: %v", err)
	}
	defer reader.Close()

	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress: %v", err)
	}

	if !bytes.Equal(input, decompressed) {
		t.Error("Decompressed data does not match original")
	}
}

func TestBrotliData(t *testing.T) {
	input := []byte("Hello, World! This is a test string for compression.")
	compressed := brotliData(input)

	// Verify it's actually compressed (should be different)
	if bytes.Equal(input, compressed) {
		t.Error("brotliData() did not compress the data")
	}

	// Verify we can decompress it back
	reader := brotli.NewReader(bytes.NewReader(compressed))
	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress brotli data: %v", err)
	}

	if !bytes.Equal(input, decompressed) {
		t.Error("Decompressed brotli data does not match original")
	}
}

func TestTemplateRoute(t *testing.T) {
	// Set up test environment
	originalAppEnv := appEnv
	appEnv = map[string]string{
		"SITE_NAME": "Test Site",
		"DEBUG":     "true",
	}
	defer func() {
		appEnv = originalAppEnv
	}()

	template := []byte(`<html><body><h1>{{.Env.SITE_NAME}}</h1><script>window.env = {{.Json}};</script></body></html>`)
	result, err := templateRoute("test.html", template)

	if err != nil {
		t.Fatalf("templateRoute() error = %v", err)
	}

	resultStr := string(result)
	if !strings.Contains(resultStr, "Test Site") {
		t.Error("templateRoute() did not substitute SITE_NAME")
	}

	if !strings.Contains(resultStr, `"SITE_NAME":"Test Site"`) {
		t.Error("templateRoute() did not include JSON environment")
	}
}

func TestGetAcceptedEncoding(t *testing.T) {
	tests := []struct {
		name           string
		acceptEncoding string
		expected       int
	}{
		{
			name:           "brotli preferred",
			acceptEncoding: "gzip, deflate, br",
			expected:       2,
		},
		{
			name:           "gzip fallback",
			acceptEncoding: "gzip, deflate",
			expected:       1,
		},
		{
			name:           "no compression",
			acceptEncoding: "identity",
			expected:       0,
		},
		{
			name:           "empty header",
			acceptEncoding: "",
			expected:       0,
		},
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

func setupTestFiles(t testing.TB) string {
	tempDir, err := os.MkdirTemp("", "nano-web-test")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}

	publicDir := filepath.Join(tempDir, "public")
	if err := os.MkdirAll(publicDir, 0755); err != nil {
		t.Fatalf("Failed to create public dir: %v", err)
	}

	// Create test files
	files := map[string]string{
		"index.html":      `<html><body><h1>{{.Env.SITE_NAME}}</h1></body></html>`,
		"style.css":       `body { font-family: Arial; }`,
		"script.js":       `console.log("Hello from {{.Env.SITE_NAME}}");`,
		"data.json":       `{"site": "{{.Env.SITE_NAME}}"}`,
		"image.png":       "fake png data",
		"subdir/sub.html": `<html><body><h2>Subdirectory</h2></body></html>`,
	}

	for path, content := range files {
		fullPath := filepath.Join(publicDir, path)
		dir := filepath.Dir(fullPath)
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatalf("Failed to create dir %s: %v", dir, err)
		}
		if err := os.WriteFile(fullPath, []byte(content), 0644); err != nil {
			t.Fatalf("Failed to write file %s: %v", fullPath, err)
		}
	}

	return tempDir
}

func TestMakeRoute(t *testing.T) {
	tempDir := setupTestFiles(t)
	defer os.RemoveAll(tempDir)

	// Set up test environment
	originalAppEnv := appEnv
	appEnv = map[string]string{"SITE_NAME": "Test Site"}
	defer func() {
		appEnv = originalAppEnv
	}()

	publicDir := filepath.Join(tempDir, "public")

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
			path:        filepath.Join(publicDir, "index.html"),
			expectError: false,
			contentType: []byte("text/html"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "CSS file with templating",
			path:        filepath.Join(publicDir, "style.css"),
			expectError: false,
			contentType: []byte("text/css"),
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "PNG file without compression",
			path:        filepath.Join(publicDir, "image.png"),
			expectError: false,
			contentType: []byte("image/png"),
			checkGzip:   false,
			checkBrotli: false,
		},
		{
			name:        "Non-existent file",
			path:        filepath.Join(publicDir, "nonexistent.txt"),
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			route, err := makeRoute(tt.path)

			if tt.expectError {
				if err == nil {
					t.Error("makeRoute() expected error but got none")
				}
				return
			}

			if err != nil {
				t.Fatalf("makeRoute() error = %v", err)
			}

			if route == nil {
				t.Fatal("makeRoute() returned nil route")
			}

			if !bytes.Equal(route.ContentType, tt.contentType) {
				t.Errorf("makeRoute() ContentType = %s, want %s", route.ContentType, tt.contentType)
			}

			if route.Content.PlainLen == 0 {
				t.Error("makeRoute() Plain content is empty")
			}

			if tt.checkGzip {
				if route.Content.GzipLen == 0 {
					t.Error("makeRoute() Gzip content should be present but is empty")
				}
			} else {
				if route.Content.GzipLen != 0 {
					t.Error("makeRoute() Gzip content should not be present")
				}
			}

			if tt.checkBrotli {
				if route.Content.BrotliLen == 0 {
					t.Error("makeRoute() Brotli content should be present but is empty")
				}
			} else {
				if route.Content.BrotliLen != 0 {
					t.Error("makeRoute() Brotli content should not be present")
				}
			}

			if len(route.LastModified) == 0 {
				t.Error("makeRoute() LastModified should not be empty")
			}
		})
	}
}

func TestHandler(t *testing.T) {
	tempDir := setupTestFiles(t)
	defer os.RemoveAll(tempDir)

	// Change to temp directory and set up routes
	originalDir, _ := os.Getwd()
	os.Chdir(tempDir)
	defer os.Chdir(originalDir)

	// Override global variables for testing
	originalPublicDir := publicDir
	originalRoutes := routes
	originalAppEnv := appEnv
	originalLogRequests := logRequests
	originalSpaMode := spaMode

	publicDir = "public"
	routes = &Routes{m: make(map[string]*Route)}
	appEnv = map[string]string{"SITE_NAME": "Test Site"}
	logRequests = false
	spaMode = false

	defer func() {
		publicDir = originalPublicDir
		routes = originalRoutes
		appEnv = originalAppEnv
		logRequests = originalLogRequests
		spaMode = originalSpaMode
	}()

	// Reset counters
	atomic.StoreUint64(&requestCount, 0)
	atomic.StoreUint64(&errorCount, 0)

	// Populate routes
	populateRoutes()

	tests := []struct {
		name           string
		path           string
		method         string
		acceptEncoding string
		setSpaMode     bool
		expectedStatus int
		expectedType   string
	}{
		{
			name:           "serve index.html",
			path:           "/",
			method:         "GET",
			acceptEncoding: "",
			expectedStatus: 200,
			expectedType:   "text/html",
		},
		{
			name:           "serve index.html with gzip",
			path:           "/",
			method:         "GET",
			acceptEncoding: "gzip",
			expectedStatus: 200,
			expectedType:   "text/html",
		},
		{
			name:           "serve CSS file",
			path:           "/style.css",
			method:         "GET",
			acceptEncoding: "",
			expectedStatus: 200,
			expectedType:   "text/css",
		},
		{
			name:           "serve PNG file",
			path:           "/image.png",
			method:         "GET",
			acceptEncoding: "",
			expectedStatus: 200,
			expectedType:   "image/png",
		},
		{
			name:           "404 without SPA mode",
			path:           "/nonexistent",
			method:         "GET",
			acceptEncoding: "",
			setSpaMode:     false,
			expectedStatus: 404,
		},
		{
			name:           "404 with SPA mode serves index",
			path:           "/nonexistent",
			method:         "GET",
			acceptEncoding: "",
			setSpaMode:     true,
			expectedStatus: 200,
			expectedType:   "text/html",
		},
		{
			name:           "health check",
			path:           "/health",
			method:         "GET",
			acceptEncoding: "",
			expectedStatus: 200,
			expectedType:   "application/json",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Set SPA mode if specified
			spaMode = tt.setSpaMode

			// Create request context
			ctx := &fasthttp.RequestCtx{}
			ctx.Request.SetRequestURI(tt.path)
			ctx.Request.Header.SetMethod(tt.method)
			if tt.acceptEncoding != "" {
				ctx.Request.Header.Set("Accept-Encoding", tt.acceptEncoding)
			}

			// Call handler
			handler(ctx)

			// Check status code
			if ctx.Response.StatusCode() != tt.expectedStatus {
				t.Errorf("handler() status = %v, want %v", ctx.Response.StatusCode(), tt.expectedStatus)
			}

			// Check content type for successful responses
			if tt.expectedStatus == 200 && tt.expectedType != "" {
				contentType := string(ctx.Response.Header.Peek("Content-Type"))
				if contentType != tt.expectedType {
					t.Errorf("handler() Content-Type = %v, want %v", contentType, tt.expectedType)
				}
			}

			// Check that server header is set
			server := string(ctx.Response.Header.Peek("Server"))
			if server != "nano-web" {
				t.Errorf("handler() Server header = %v, want nano-web", server)
			}

			// Check encoding header for compressed responses
			if tt.acceptEncoding == "gzip" && tt.expectedStatus == 200 {
				encoding := string(ctx.Response.Header.Peek("Content-Encoding"))
				if encoding != "gzip" {
					t.Errorf("handler() Content-Encoding = %v, want gzip", encoding)
				}
			}
		})
	}
}

func TestPopulateRoutes(t *testing.T) {
	tempDir := setupTestFiles(t)
	defer os.RemoveAll(tempDir)

	// Change to temp directory
	originalDir, _ := os.Getwd()
	os.Chdir(tempDir)
	defer os.Chdir(originalDir)

	// Override global variables for testing
	originalPublicDir := publicDir
	originalRoutes := routes
	originalAppEnv := appEnv

	publicDir = "public"
	routes = &Routes{m: make(map[string]*Route)}
	appEnv = map[string]string{"SITE_NAME": "Test Site"}

	defer func() {
		publicDir = originalPublicDir
		routes = originalRoutes
		appEnv = originalAppEnv
	}()

	populateRoutes()

	// Check that routes were created
	routes.RLock()
	routeCount := len(routes.m)
	routes.RUnlock()

	if routeCount == 0 {
		t.Error("populateRoutes() created no routes")
	}

	// Check specific routes
	expectedRoutes := []string{"/", "/index.html", "/style.css", "/script.js", "/data.json", "/image.png", "/subdir/sub.html"}

	for _, path := range expectedRoutes {
		route, exists := getRoute(path)
		if !exists {
			t.Errorf("populateRoutes() missing route for %s", path)
		} else if route == nil {
			t.Errorf("populateRoutes() route for %s is nil", path)
		}
	}
}

func BenchmarkHandler(b *testing.B) {
	tempDir := setupTestFiles(b)
	defer os.RemoveAll(tempDir)

	// Change to temp directory and set up routes
	originalDir, _ := os.Getwd()
	os.Chdir(tempDir)
	defer os.Chdir(originalDir)

	// Override global variables for testing
	originalPublicDir := publicDir
	originalRoutes := routes
	originalAppEnv := appEnv
	originalLogRequests := logRequests

	publicDir = "public"
	routes = &Routes{m: make(map[string]*Route)}
	appEnv = map[string]string{"SITE_NAME": "Test Site"}
	logRequests = false

	defer func() {
		publicDir = originalPublicDir
		routes = originalRoutes
		appEnv = originalAppEnv
		logRequests = originalLogRequests
	}()

	populateRoutes()

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		ctx := &fasthttp.RequestCtx{}
		ctx.Request.SetRequestURI("/")
		ctx.Request.Header.SetMethod("GET")
		ctx.Request.Header.Set("Accept-Encoding", "gzip")
		for pb.Next() {
			ctx.Response.Reset()
			handler(ctx)
		}
	})
}

func BenchmarkGzipCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is a test string for compression. ", 100))

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = gzipData(data)
	}
}

func BenchmarkBrotliCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is a test string for compression. ", 100))

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = brotliData(data)
	}
}
