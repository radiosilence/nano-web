package main

import (
	"bytes"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"text/template"
	"time"

	"github.com/alecthomas/kong"
	"github.com/andybalholm/brotli"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/valyala/fasthttp"
)

type Route struct {
	Content      Content
	ContentType  []byte
	LastModified []byte
}

type Content struct {
	Plain        []byte
	Gzip         []byte
	Brotli       []byte
	PlainLen     int
	GzipLen      int
	BrotliLen    int
}

type Routes struct {
	sync.RWMutex
	m map[string]*Route
}

var (
	appEnv map[string]string
	routes *Routes
	
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
	
	// Request counter for stats
	requestCount uint64
	errorCount   uint64
	
	// Buffer pools
	bufferPool = sync.Pool{
		New: func() interface{} {
			return &bytes.Buffer{}
		},
	}
)

// CLI structure defining all commands and flags
type CLI struct {
	// Commands
	Serve       ServeCmd       `cmd:"" help:"Start the web server"`
	HealthCheck HealthCheckCmd `cmd:"" help:"Perform health check and exit"`
	Version     VersionCmd     `cmd:"" help:"Show version information"`
}

type ServeCmd struct {
	// Positional argument for the directory to serve
	PublicDir string `arg:"" optional:"" help:"Directory containing static files to serve" default:"public"`
	
	// Server configuration
	Port         int    `short:"p" long:"port" help:"Port to listen on" default:"80" env:"PORT"`
	SpaMode      bool   `short:"s" long:"spa-mode" help:"Enable SPA mode (serve index.html for 404s)" env:"SPA_MODE"`
	ConfigPrefix string `long:"config-prefix" help:"Prefix for runtime environment variable injection" default:"VITE_" env:"CONFIG_PREFIX"`
	
	// Logging configuration
	LogLevel    string `long:"log-level" help:"Logging level" enum:"debug,info,warn,error" default:"info" env:"LOG_LEVEL"`
	LogFormat   string `long:"log-format" help:"Log format" enum:"json,console" default:"json" env:"LOG_FORMAT"`
	LogRequests bool   `long:"log-requests" help:"Enable request logging" default:"true" env:"LOG_REQUESTS"`
}

type HealthCheckCmd struct{}

func (h *HealthCheckCmd) Run() error {
	fmt.Println(`{"status":"healthy","timestamp":"` + time.Now().UTC().Format(time.RFC3339) + `"}`)
	return nil
}

type VersionCmd struct{}

func (v *VersionCmd) Run() error {
	fmt.Println("nano-web v1.0.0")
	fmt.Println("Ultra-fast static file server built with Go")
	fmt.Println("Repository: https://github.com/radiosilence/nano-web")
	return nil
}

func (s *ServeCmd) Run() error {
	// Initialize global variables from CLI options
	appEnv = getAppEnv(s.ConfigPrefix)
	routes = &Routes{
		m: make(map[string]*Route),
	}

	// Configure logger
	if s.LogFormat == "json" {
		log.Logger = zerolog.New(os.Stdout).With().Timestamp().Logger()
	} else {
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stdout, TimeFormat: time.RFC3339})
	}

	// Set log level
	switch s.LogLevel {
	case "debug":
		zerolog.SetGlobalLevel(zerolog.DebugLevel)
	case "info":
		zerolog.SetGlobalLevel(zerolog.InfoLevel)
	case "warn":
		zerolog.SetGlobalLevel(zerolog.WarnLevel)
	case "error":
		zerolog.SetGlobalLevel(zerolog.ErrorLevel)
	default:
		zerolog.SetGlobalLevel(zerolog.InfoLevel)
	}

	addr := fmt.Sprintf(":%d", s.Port)

	log.Info().
		Str("version", "nano-web v1.0.0").
		Int("port", s.Port).
		Str("public_dir", s.PublicDir).
		Bool("spa_mode", s.SpaMode).
		Bool("log_requests", s.LogRequests).
		Str("config_prefix", s.ConfigPrefix).
		Msg("starting nano-web server")

	populateRoutes(s.PublicDir)
	
	// Configure server for maximum performance
	server := &fasthttp.Server{
		Handler:                       func(ctx *fasthttp.RequestCtx) { handler(ctx, s) },
		Name:                         "nano-web",
		ReadBufferSize:               16 * 1024, // 16KB
		WriteBufferSize:              16 * 1024, // 16KB
		MaxConnsPerIP:                1000,
		MaxRequestsPerConn:           1000,
		MaxRequestBodySize:           1 * 1024 * 1024, // 1MB
		DisableKeepalive:             false,
		TCPKeepalive:                 true,
		TCPKeepalivePeriod:           30 * time.Second,
		ReduceMemoryUsage:            false, // Keep false for performance
		GetOnly:                      true,  // Only serve GET requests
		DisablePreParseMultipartForm: true,
		LogAllErrors:                 false,
		DisableHeaderNamesNormalizing: true, // Performance optimization
		NoDefaultServerHeader:        true,
		StreamRequestBody:            false,
	}

	log.Info().Str("addr", addr).Msg("server listening")
	if err := server.ListenAndServe(addr); err != nil {
		log.Fatal().Err(err).Msg("server failed to start")
	}
	
	return nil
}

func getAppEnv(prefix string) map[string]string {
	appEnv := make(map[string]string)
	for _, env := range os.Environ() {
		if idx := strings.IndexByte(env, '='); idx > 0 {
			key := env[:idx]
			value := env[idx+1:]
			if strings.HasPrefix(key, prefix) {
				appEnv[key[len(prefix):]] = value
			}
		}
	}
	return appEnv
}

var mimetypes = map[string][]byte{
	".html": []byte("text/html"),
	".css":  []byte("text/css"),
	".js":   []byte("text/javascript"),
	".json": []byte("application/json"),
	".xml":  []byte("application/xml"),
	".pdf":  []byte("application/pdf"),
	".zip":  []byte("application/zip"),
	".doc":  []byte("application/msword"),
	".eot":  []byte("application/vnd.ms-fontobject"),
	".otf":  []byte("font/otf"),
	".ttf":  []byte("font/ttf"),
	".woff": []byte("font/woff"),
	".woff2":[]byte("font/woff2"),
	".gif":  []byte("image/gif"),
	".jpeg": []byte("image/jpeg"),
	".jpg":  []byte("image/jpeg"),
	".png":  []byte("image/png"),
	".svg":  []byte("image/svg+xml"),
	".ico":  []byte("image/x-icon"),
	".webp": []byte("image/webp"),
	".mp4":  []byte("video/mp4"),
	".webm": []byte("video/webm"),
	".wav":  []byte("audio/wav"),
	".mp3":  []byte("audio/mpeg"),
	".ogg":  []byte("audio/ogg"),
	".csv":  []byte("text/csv"),
	".txt":  []byte("text/plain"),
}

var defaultMimetype = []byte("application/octet-stream")

func getMimetype(path string) []byte {
	if idx := strings.LastIndexByte(path, '.'); idx > 0 {
		ext := strings.ToLower(path[idx:])
		if mimetype, ok := mimetypes[ext]; ok {
			return mimetype
		}
	}
	return defaultMimetype
}

type TemplateData struct {
	Env         map[string]string `json:"env"`
	Json        string            `json:"json"`
	EscapedJson string            `json:"escapedJson"`
}

func templateRoute(name string, content []byte) ([]byte, error) {
	tmpl, err := template.New(name).Parse(string(content))
	if err != nil {
		return nil, err
	}
	
	jsonString, err := json.Marshal(appEnv)
	if err != nil {
		return nil, err
	}
	
	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()
	
	err = tmpl.Execute(buffer, &TemplateData{
		Env:         appEnv,
		Json:        string(jsonString),
		EscapedJson: strings.Replace(string(jsonString), "\"", "\\\"", -1),
	})
	if err != nil {
		return nil, err
	}
	
	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result, nil
}

func shouldTemplate(mimetype []byte) bool {
	return bytes.Equal(mimetype, []byte("text/html")) ||
		bytes.Equal(mimetype, []byte("text/css")) ||
		bytes.Equal(mimetype, []byte("text/javascript")) ||
		bytes.Equal(mimetype, []byte("application/json"))
}

func shouldCompress(mimetype []byte) bool {
	return shouldTemplate(mimetype)
}

func gzipData(dat []byte) []byte {
	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()
	
	w := gzip.NewWriter(buffer)
	w.Write(dat)
	w.Close()
	
	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result
}

func brotliData(dat []byte) []byte {
	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()
	
	w := brotli.NewWriter(buffer)
	w.Write(dat)
	w.Close()
	
	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result
}

func makeRoute(path string) (*Route, error) {
	dat, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	info, err := os.Stat(path)
	if err != nil {
		return nil, err
	}

	mimetype := getMimetype(path)
	
	if shouldTemplate(mimetype) {
		dat, err = templateRoute(path, dat)
		if err != nil {
			return nil, err
		}
	}

	content := Content{
		Plain:    dat,
		PlainLen: len(dat),
	}

	if shouldCompress(mimetype) {
		content.Gzip = gzipData(dat)
		content.GzipLen = len(content.Gzip)
		content.Brotli = brotliData(dat)
		content.BrotliLen = len(content.Brotli)
	}

	lastModified := info.ModTime().Format(http.TimeFormat)
	
	return &Route{
		Content:      content,
		ContentType:  mimetype,
		LastModified: []byte(lastModified),
	}, nil
}

func populateRoutes(publicDir string) {
	_, err := os.Stat(publicDir)
	if err != nil {
		cwd, _ := os.Getwd()
		log.Fatal().Str("cwd", cwd).Str("public_dir", publicDir).Msg("public directory not found")
	}

	routeCount := 0
	filepath.Walk(publicDir, func(path string, info os.FileInfo, err error) error {
		if info.IsDir() {
			return nil
		}
		
		urlPath := strings.Replace(path, publicDir, "", 1)
		
		route, err := makeRoute(path)
		if err != nil {
			log.Error().Err(err).Str("url_path", urlPath).Str("file_path", path).Msg("error making route")
			return nil
		}

		routes.Lock()
		routes.m[urlPath] = route
		routeCount++
		
		if info.Name() == "index.html" {
			indexUrlPath := strings.Replace(urlPath, "/index.html", "", 1)
			if indexUrlPath == "" {
				indexUrlPath = "/"
			}
			log.Debug().Str("index_path", indexUrlPath).Str("file_path", path).Msg("adding index route")
			routes.m[indexUrlPath] = route
			routes.m[indexUrlPath+"/"] = route
			routeCount += 2
		}
		routes.Unlock()
		
		log.Debug().Str("url_path", urlPath).Str("file_path", path).Msg("adding route")
		return nil
	})

	log.Info().Int("route_count", routeCount).Msg("routes populated successfully")
}

func getRoute(path string) (*Route, bool) {
	routes.RLock()
	route, exists := routes.m[path]
	routes.RUnlock()
	return route, exists
}

func getAcceptedEncoding(ctx *fasthttp.RequestCtx) int {
	acceptHeader := ctx.Request.Header.Peek("Accept-Encoding")
	
	// Check for br first as it's usually better compression
	if bytes.Contains(acceptHeader, brEncoding) {
		return 2 // br
	}
	if bytes.Contains(acceptHeader, gzipEncoding) {
		return 1 // gzip
	}
	return 0 // none
}

func healthCheckHandler(ctx *fasthttp.RequestCtx) {
	ctx.Response.Header.SetBytesKV(contentTypeKey, []byte("application/json"))
	ctx.Response.Header.SetBytesKV(serverKey, serverValue)
	ctx.SetStatusCode(fasthttp.StatusOK)
	fmt.Fprintf(ctx, `{"status":"healthy","timestamp":"%s","requests":%d,"errors":%d}`, 
		time.Now().UTC().Format(time.RFC3339),
		atomic.LoadUint64(&requestCount),
		atomic.LoadUint64(&errorCount))
}

func handler(ctx *fasthttp.RequestCtx, s *ServeCmd) {
	atomic.AddUint64(&requestCount, 1)
	
	start := time.Now()
	path := ctx.Path()
	
	// Fast path for health checks
	if bytes.Equal(path, healthPath) || bytes.Equal(path, altHealthPath) {
		healthCheckHandler(ctx)
		return
	}
	
	pathStr := string(path)
	route, exists := getRoute(pathStr)
	
	if !exists {
		if s.SpaMode {
			route, exists = getRoute("/")
			if !exists {
				atomic.AddUint64(&errorCount, 1)
				duration := time.Since(start)
				if s.LogRequests {
					log.Warn().
						Str("method", string(ctx.Method())).
						Str("path", pathStr).
						Str("user_agent", string(ctx.UserAgent())).
						Int("status", fasthttp.StatusNotFound).
						Dur("duration_ms", duration).
						Msg("request not found")
				}
				ctx.Error("Not Found", fasthttp.StatusNotFound)
				ctx.Response.Header.SetBytesKV(serverKey, serverValue)
				return
			}
		} else {
			atomic.AddUint64(&errorCount, 1)
			duration := time.Since(start)
			if s.LogRequests {
				log.Warn().
					Str("method", string(ctx.Method())).
					Str("path", pathStr).
					Str("user_agent", string(ctx.UserAgent())).
					Int("status", fasthttp.StatusNotFound).
					Dur("duration_ms", duration).
					Msg("request not found")
			}
			ctx.Error("Not Found", fasthttp.StatusNotFound)
			ctx.Response.Header.SetBytesKV(serverKey, serverValue)
			return
		}
	}

	// Set headers efficiently
	ctx.Response.Header.SetBytesKV(contentTypeKey, route.ContentType)
	ctx.Response.Header.SetBytesKV(serverKey, serverValue)
	ctx.Response.Header.SetBytesKV(lastModifiedKey, route.LastModified)
	
	// Get content based on encoding
	encoding := getAcceptedEncoding(ctx)
	var content []byte
	var encodingHeader []byte
	
	switch encoding {
	case 2: // br
		if route.Content.BrotliLen > 0 {
			content = route.Content.Brotli
			encodingHeader = brEncoding
		} else {
			content = route.Content.Plain
		}
	case 1: // gzip
		if route.Content.GzipLen > 0 {
			content = route.Content.Gzip
			encodingHeader = gzipEncoding
		} else {
			content = route.Content.Plain
		}
	default:
		content = route.Content.Plain
	}
	
	if encodingHeader != nil {
		ctx.Response.Header.SetBytesKV(contentEncoding, encodingHeader)
	}
	
	ctx.SetStatusCode(fasthttp.StatusOK)
	ctx.Response.SetBody(content)

	if s.LogRequests {
		duration := time.Since(start)
		log.Info().
			Str("method", string(ctx.Method())).
			Str("path", pathStr).
			Str("user_agent", string(ctx.UserAgent())).
			Str("content_type", string(route.ContentType)).
			Str("encoding", string(encodingHeader)).
			Int("status", fasthttp.StatusOK).
			Int("content_length", len(content)).
			Dur("duration_ms", duration).
			Msg("request served")
	}
}

func main() {
	cli := &CLI{}
	
	ctx := kong.Parse(cli,
		kong.Name("nano-web"),
		kong.Description("🚀 Ultra-fast static file server for SPAs and static content\n\nBuilt on FastHTTP, nano-web is designed for maximum performance and minimal resource usage.\nPerfect for containerized deployments, edge computing, and unikernel environments."),
		kong.UsageOnError(),
		kong.ConfigureHelp(kong.HelpOptions{
			Compact: true,
			Summary: true,
		}),
		kong.Vars{
			"version": "1.0.0",
		},
	)
	
	err := ctx.Run()
	ctx.FatalIfErrorf(err)
}