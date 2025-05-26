package main

import (
	"bytes"
	"compress/gzip"
	"io"
	"os"
	"path/filepath"
	"strings"
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
	// Set up test environment variables
	os.Setenv("VITE_API_URL", "https://api.example.com")
	os.Setenv("VITE_DEBUG", "true")
	os.Setenv("OTHER_VAR", "should_not_be_included")
	defer func() {
		os.Unsetenv("VITE_API_URL")
		os.Unsetenv("VITE_DEBUG")
		os.Unsetenv("OTHER_VAR")
	}()

	// Test with default prefix
	os.Unsetenv("CONFIG_PREFIX")
	appEnv := getAppEnv()

	if appEnv["API_URL"] != "https://api.example.com" {
		t.Errorf("Expected API_URL to be 'https://api.example.com', got %v", appEnv["API_URL"])
	}

	if appEnv["DEBUG"] != "true" {
		t.Errorf("Expected DEBUG to be 'true', got %v", appEnv["DEBUG"])
	}

	if _, exists := appEnv["OTHER_VAR"]; exists {
		t.Errorf("OTHER_VAR should not be included in appEnv")
	}

	// Test with custom prefix
	os.Setenv("CONFIG_PREFIX", "REACT_APP_")
	os.Setenv("REACT_APP_VERSION", "1.0.0")
	defer func() {
		os.Unsetenv("CONFIG_PREFIX")
		os.Unsetenv("REACT_APP_VERSION")
	}()

	appEnv = getAppEnv()
	if appEnv["VERSION"] != "1.0.0" {
		t.Errorf("Expected VERSION to be '1.0.0', got %v", appEnv["VERSION"])
	}
}

func TestGetMimetype(t *testing.T) {
	tests := []struct {
		ext      string
		expected string
	}{
		{".html", "text/html"},
		{".css", "text/css"},
		{".js", "text/javascript"},
		{".json", "application/json"},
		{".png", "image/png"},
		{".jpg", "image/jpeg"},
		{".jpeg", "image/jpeg"},
		{".gif", "image/gif"},
		{".svg", "image/svg+xml"},
		{".ico", "image/x-icon"},
		{".webp", "image/webp"},
		{".pdf", "application/pdf"},
		{".zip", "application/zip"},
		{".mp4", "video/mp4"},
		{".webm", "video/webm"},
		{".mp3", "audio/mpeg"},
		{".wav", "audio/wav"},
		{".ogg", "audio/ogg"},
		{".txt", "text/plain"},
		{".csv", "text/csv"},
		{".xml", "application/xml"},
		{".ttf", "font/ttf"},
		{".woff", "font/woff"},
		{".woff2", "font/woff2"},
		{".unknown", "application/octet-stream"},
	}

	for _, tt := range tests {
		t.Run(tt.ext, func(t *testing.T) {
			result := getMimetype(tt.ext)
			if result != tt.expected {
				t.Errorf("getMimetype(%s) = %v, want %v", tt.ext, result, tt.expected)
			}
		})
	}
}

func TestTemplateType(t *testing.T) {
	tests := []struct {
		mimetype string
		expected bool
	}{
		{"text/html", true},
		{"text/css", true},
		{"text/javascript", true},
		{"application/json", true},
		{"image/png", false},
		{"application/pdf", false},
		{"video/mp4", false},
	}

	for _, tt := range tests {
		t.Run(tt.mimetype, func(t *testing.T) {
			result := templateType(tt.mimetype)
			if result != tt.expected {
				t.Errorf("templateType(%s) = %v, want %v", tt.mimetype, result, tt.expected)
			}
		})
	}
}

func TestCompressedType(t *testing.T) {
	tests := []struct {
		mimetype string
		expected bool
	}{
		{"text/html", true},
		{"text/css", true},
		{"text/javascript", true},
		{"application/json", true},
		{"image/png", false},
		{"application/pdf", false},
		{"video/mp4", false},
	}

	for _, tt := range tests {
		t.Run(tt.mimetype, func(t *testing.T) {
			result := compressedType(tt.mimetype)
			if result != tt.expected {
				t.Errorf("compressedType(%s) = %v, want %v", tt.mimetype, result, tt.expected)
			}
		})
	}
}

func TestGzipData(t *testing.T) {
	input := []byte("Hello, World!")
	compressed := gzipData(input)

	// Decompress to verify
	reader, err := gzip.NewReader(bytes.NewReader(compressed))
	if err != nil {
		t.Fatalf("Failed to create gzip reader: %v", err)
	}
	defer reader.Close()

	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress: %v", err)
	}

	if !bytes.Equal(decompressed, input) {
		t.Errorf("Decompressed data does not match input. Got %s, want %s", decompressed, input)
	}
}

func TestBrotliData(t *testing.T) {
	input := []byte("Hello, World!")
	compressed := brotliData(input)

	// Decompress to verify
	reader := brotli.NewReader(bytes.NewReader(compressed))
	decompressed, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("Failed to decompress: %v", err)
	}

	if !bytes.Equal(decompressed, input) {
		t.Errorf("Decompressed data does not match input. Got %s, want %s", decompressed, input)
	}
}

func TestTemplateRoute(t *testing.T) {
	// Set up test environment
	originalAppEnv := appEnv
	appEnv = map[string]string{
		"API_URL": "https://test.example.com",
		"DEBUG":   "true",
	}
	defer func() { appEnv = originalAppEnv }()

	template := `<html><head><script>window.ENV = {{.Json}};</script></head></html>`
	
	result, err := templateRoute("test", template)
	if err != nil {
		t.Fatalf("templateRoute() error = %v", err)
	}

	if !strings.Contains(result, "https://test.example.com") {
		t.Errorf("Template result should contain API_URL value")
	}

	if !strings.Contains(result, "window.ENV") {
		t.Errorf("Template result should contain templated content")
	}
}

func TestGetAcceptedEncoding(t *testing.T) {
	tests := []struct {
		name           string
		acceptEncoding string
		expected       string
	}{
		{
			name:           "brotli preferred",
			acceptEncoding: "gzip, deflate, br",
			expected:       "br",
		},
		{
			name:           "gzip fallback",
			acceptEncoding: "gzip, deflate",
			expected:       "gzip",
		},
		{
			name:           "no compression",
			acceptEncoding: "identity",
			expected:       "",
		},
		{
			name:           "empty header",
			acceptEncoding: "",
			expected:       "",
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

func TestGetEncodedContent(t *testing.T) {
	plainData := []byte("Hello, World!")
	gzipData := gzipData(plainData)
	brotliData := brotliData(plainData)

	content := Content{
		Plain:  plainData,
		Gzip:   gzipData,
		Brotli: brotliData,
	}

	tests := []struct {
		name             string
		acceptedEncoding string
		expectedEncoding string
		expectedContent  []byte
	}{
		{
			name:             "brotli encoding",
			acceptedEncoding: "br",
			expectedEncoding: "br",
			expectedContent:  brotliData,
		},
		{
			name:             "gzip encoding",
			acceptedEncoding: "gzip",
			expectedEncoding: "gzip",
			expectedContent:  gzipData,
		},
		{
			name:             "no encoding",
			acceptedEncoding: "",
			expectedEncoding: "",
			expectedContent:  plainData,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			encoding, data := getEncodedContent(tt.acceptedEncoding, content)
			
			if encoding != tt.expectedEncoding {
				t.Errorf("getEncodedContent() encoding = %v, want %v", encoding, tt.expectedEncoding)
			}

			if !bytes.Equal(data, tt.expectedContent) {
				t.Errorf("getEncodedContent() content mismatch")
			}
		})
	}
}

func setupTestFiles(t testing.TB) string {
	// Create temporary directory
	tempDir, err := os.MkdirTemp("", "nano-web-test")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}

	// Create test public directory
	publicDir := filepath.Join(tempDir, "public")
	err = os.MkdirAll(publicDir, 0755)
	if err != nil {
		t.Fatalf("Failed to create public dir: %v", err)
	}

	// Create test files
	testFiles := map[string]string{
		"index.html":    `<html><head><script>window.ENV = {{.Json}};</script></head><body><h1>Hello {{.Env.SITE_NAME}}</h1></body></html>`,
		"style.css":     `body { color: red; }`,
		"script.js":     `console.log("Hello from {{.Env.SITE_NAME}}");`,
		"data.json":     `{"message": "Hello {{.Env.SITE_NAME}}"}`,
		"image.png":     "fake-png-data",
		"doc.pdf":       "fake-pdf-data",
		"subdir/page.html": `<html><body><h2>Subpage</h2></body></html>`,
	}

	for file, content := range testFiles {
		filePath := filepath.Join(publicDir, file)
		dir := filepath.Dir(filePath)
		
		err := os.MkdirAll(dir, 0755)
		if err != nil {
			t.Fatalf("Failed to create directory %s: %v", dir, err)
		}

		err = os.WriteFile(filePath, []byte(content), 0644)
		if err != nil {
			t.Fatalf("Failed to write file %s: %v", filePath, err)
		}
	}

	return tempDir
}

func TestMakeRoute(t *testing.T) {
	tempDir := setupTestFiles(t)
	defer os.RemoveAll(tempDir)

	// Set up test environment
	originalAppEnv := appEnv
	appEnv = map[string]string{
		"SITE_NAME": "Test Site",
	}
	defer func() { appEnv = originalAppEnv }()

	// Change to temp directory
	originalDir, _ := os.Getwd()
	os.Chdir(tempDir)
	defer os.Chdir(originalDir)

	tests := []struct {
		name        string
		path        string
		expectError bool
		contentType string
		checkGzip   bool
		checkBrotli bool
	}{
		{
			name:        "HTML file with templating",
			path:        "public/index.html",
			expectError: false,
			contentType: "text/html",
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "CSS file with templating",
			path:        "public/style.css",
			expectError: false,
			contentType: "text/css",
			checkGzip:   true,
			checkBrotli: true,
		},
		{
			name:        "PNG file without compression",
			path:        "public/image.png",
			expectError: false,
			contentType: "image/png",
			checkGzip:   false,
			checkBrotli: false,
		},
		{
			name:        "Non-existent file",
			path:        "public/nonexistent.html",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			route, err := makeRoute(tt.path)

			if tt.expectError {
				if err == nil {
					t.Errorf("makeRoute() expected error but got none")
				}
				return
			}

			if err != nil {
				t.Fatalf("makeRoute() error = %v", err)
			}

			if route.ContentType != tt.contentType {
				t.Errorf("makeRoute() ContentType = %v, want %v", route.ContentType, tt.contentType)
			}

			if tt.checkGzip && len(route.Content.Gzip) == 0 {
				t.Errorf("makeRoute() should have gzip content for %s", tt.path)
			}

			if tt.checkBrotli && len(route.Content.Brotli) == 0 {
				t.Errorf("makeRoute() should have brotli content for %s", tt.path)
			}

			if !tt.checkGzip && len(route.Content.Gzip) > 0 {
				t.Errorf("makeRoute() should not have gzip content for %s", tt.path)
			}

			if !tt.checkBrotli && len(route.Content.Brotli) > 0 {
				t.Errorf("makeRoute() should not have brotli content for %s", tt.path)
			}

			// Check if templating worked for templatable files
			if strings.Contains(tt.path, ".html") || strings.Contains(tt.path, ".css") || strings.Contains(tt.path, ".js") || strings.Contains(tt.path, ".json") {
				content := string(route.Content.Plain)
				if strings.Contains(content, "{{.Env.SITE_NAME}}") {
					t.Errorf("Template variables should be replaced in %s", tt.path)
				}
				if !strings.Contains(content, "Test Site") && strings.Contains(tt.path, "SITE_NAME") {
					t.Errorf("Template should contain replaced value 'Test Site' in %s", tt.path)
				}
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

	publicDir = "public"
	routes = make(map[string]Route)
	appEnv = map[string]string{"SITE_NAME": "Test Site"}
	logRequests = false // Disable request logging for tests

	defer func() {
		publicDir = originalPublicDir
		routes = originalRoutes
		appEnv = originalAppEnv
		logRequests = originalLogRequests
	}()

	// Populate routes
	populateRoutes(routes)

	tests := []struct {
		name           string
		path           string
		method         string
		acceptEncoding string
		spaMode        string
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
			spaMode:        "0",
			expectedStatus: 404,
		},
		{
			name:           "404 with SPA mode serves index",
			path:           "/nonexistent",
			method:         "GET",
			acceptEncoding: "",
			spaMode:        "1",
			expectedStatus: 200,
			expectedType:   "text/html",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Set SPA mode if specified
			if tt.spaMode != "" {
				os.Setenv("SPA_MODE", tt.spaMode)
				defer os.Unsetenv("SPA_MODE")
			}

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
					t.Errorf("handler() content-type = %v, want %v", contentType, tt.expectedType)
				}
			}

			// Check that server header is set for successful responses
			if tt.expectedStatus == 200 {
				server := string(ctx.Response.Header.Peek("Server"))
				if server != "nano-web" {
					t.Errorf("handler() server header = %v, want nano-web", server)
				}
			}

			// Check compression headers for compressed responses
			if tt.acceptEncoding == "gzip" && tt.expectedStatus == 200 && compressedType(tt.expectedType) {
				encoding := string(ctx.Response.Header.Peek("Content-Encoding"))
				if encoding != "gzip" {
					t.Errorf("handler() should set gzip encoding for compressed content")
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
	originalAppEnv := appEnv

	publicDir = "public"
	appEnv = map[string]string{"SITE_NAME": "Test Site"}

	defer func() {
		publicDir = originalPublicDir
		appEnv = originalAppEnv
	}()

	// Create new routes map
	testRoutes := make(map[string]Route)

	// Populate routes
	populateRoutes(testRoutes)

	// Check that expected routes exist
	expectedRoutes := []string{
		"/index.html",
		"/",
		"//", // index route with trailing slash
		"/style.css",
		"/script.js",
		"/data.json",
		"/image.png",
		"/doc.pdf",
		"/subdir/page.html",
	}

	for _, route := range expectedRoutes {
		if _, exists := testRoutes[route]; !exists {
			t.Errorf("Expected route %s to exist", route)
		}
	}

	// Check that index routes are properly set
	indexRoute, exists := testRoutes["/"]
	if !exists {
		t.Fatalf("Index route should exist")
	}

	indexHtmlRoute, exists := testRoutes["/index.html"]
	if !exists {
		t.Fatalf("Index.html route should exist")
	}

	// They should be the same route
	if !bytes.Equal(indexRoute.Content.Plain, indexHtmlRoute.Content.Plain) {
		t.Errorf("Index route and index.html route should have same content")
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
	routes = make(map[string]Route)
	appEnv = map[string]string{"SITE_NAME": "Test Site"}
	logRequests = false // Disable logging for benchmarks

	defer func() {
		publicDir = originalPublicDir
		routes = originalRoutes
		appEnv = originalAppEnv
		logRequests = originalLogRequests
	}()

	// Populate routes
	populateRoutes(routes)

	// Create request context
	ctx := &fasthttp.RequestCtx{}
	ctx.Request.SetRequestURI("/")
	ctx.Request.Header.SetMethod("GET")

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			ctx.Response.Reset()
			handler(ctx)
		}
	})
}

func BenchmarkGzipCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is a test string for compression benchmarks. ", 100))
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = gzipData(data)
	}
}

func BenchmarkBrotliCompression(b *testing.B) {
	data := []byte(strings.Repeat("Hello, World! This is a test string for compression benchmarks. ", 100))
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = brotliData(data)
	}
}