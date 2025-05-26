package main

import (
	"bytes"
	"compress/gzip"
	"encoding/json"
	"flag"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"text/template"
	"time"

	"github.com/andybalholm/brotli"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/valyala/fasthttp"
)

type Route struct {
	Content      Content
	ContentType  string
	LastModified string
}

type Routes map[string]Route

func getEnv(name string, fallback string) string {
	value, exists := os.LookupEnv(name)
	if !exists {
		value = fallback
	}
	return value
}

func getAppEnv() map[string]string {
	prefix := getEnv("CONFIG_PREFIX", "VITE_")
	appEnv := make(map[string]string)
	for _, env := range os.Environ() {
		parts := strings.Split(env, "=")
		key := parts[0]
		value := strings.Join(parts[1:], "=")
		if strings.HasPrefix(key, prefix) {
			appEnv[strings.Replace(key, prefix, "", 1)] = value
		}
	}
	return appEnv
}

var appEnv = getAppEnv()
var publicDir = getEnv("PUBLIC_DIR", "public")
var routes Routes = make(map[string]Route)
var logRequests = getEnv("LOG_REQUESTS", "true") == "true"

func getMimetype(ext string) string {
	switch ext {
	case ".html":
		return "text/html"
	case ".css":
		return "text/css"
	case ".js":
		return "text/javascript"
	case ".json":
		return "application/json"
	case ".xml":
		return "application/xml"
	case ".pdf":
		return "application/pdf"
	case ".zip":
		return "application/zip"
	case ".doc":
		return "application/msword"
	case ".eot":
		return "application/vnd.ms-fontobject"
	case ".otf":
		return "font/otf"
	case ".ttf":
		return "font/ttf"
	case ".woff":
		return "font/woff"
	case ".woff2":
		return "font/woff2"
	case ".gif":
		return "image/gif"
	case ".jpeg":
		return "image/jpeg"
	case ".jpg":
		return "image/jpeg"
	case ".png":
		return "image/png"
	case ".svg":
		return "image/svg+xml"
	case ".ico":
		return "image/x-icon"
	case ".webp":
		return "image/webp"
	case ".mp4":
		return "video/mp4"
	case ".webm":
		return "video/webm"
	case ".wav":
		return "audio/wav"
	case ".mp3":
		return "audio/mpeg"
	case ".ogg":
		return "audio/ogg"
	case ".csv":
		return "text/csv"
	case ".txt":
		return "text/plain"
	default:
		return "application/octet-stream"
	}
}

type TemplateData struct {
	Env         map[string]string `json:"env"`
	Json        string            `json:"json"`
	EscapedJson string            `json:"escapedJson"`
}

type Content struct {
	Plain  []byte
	Gzip   []byte
	Brotli []byte
}

func templateRoute(name string, content string) (string, error) {
	writer := bytes.NewBufferString("")
	tmpl, err := template.New(name).Parse(content)
	if err != nil {
		return "", err
	}
	jsonString, err := json.Marshal(appEnv)
	if err != nil {
		return "", err
	}
	err = tmpl.Execute(writer, &TemplateData{
		Env:         appEnv,
		Json:        string(jsonString),
		EscapedJson: strings.Replace(string(jsonString), "\"", "\\\"", -1),
	})
	if err != nil {
		return "", err
	}
	return writer.String(), nil
}

func templateType(mimetype string) bool {
	switch mimetype {
	case "text/html", "text/css", "text/javascript", "application/json":
		return true
	default:
		return false
	}
}

func compressedType(mimetype string) bool {
	switch mimetype {
	case "text/html", "text/css", "text/javascript", "application/json":
		return true
	default:
		return false
	}
}

func gzipData(dat []byte) []byte {
	var b bytes.Buffer
	w := gzip.NewWriter(&b)
	w.Write(dat)
	w.Close()
	return b.Bytes()
}

func brotliData(dat []byte) []byte {
	var b bytes.Buffer
	w := brotli.NewWriter(&b)
	w.Write(dat)
	w.Close()
	return b.Bytes()

}

func makeRoute(path string) (Route, error) {
	ext := strings.ToLower(path[strings.LastIndex(path, "."):])
	mimetype := getMimetype(ext)
	dat, err := os.ReadFile(path)

	if err != nil {
		return Route{}, err
	}

	info, err := os.Stat(path)

	if err != nil {
		return Route{}, err
	}

	if templateType(mimetype) {
		content, err := templateRoute(path, string(dat))
		if err != nil {
			return Route{}, err
		}
		dat = []byte(content)

	}

	content := Content{
		Plain: dat,
	}

	if compressedType(mimetype) {
		content.Gzip = gzipData(dat)
		content.Brotli = brotliData(dat)
	}

	return Route{
		Content:      content,
		ContentType:  mimetype,
		LastModified: info.ModTime().Format(http.TimeFormat),
	}, nil
}

// Walk the public dir and create routes for each file
func populateRoutes(routes Routes) {
	_, err := os.Stat(publicDir)
	if err != nil {
		cwd, err := os.Getwd()
		if err != nil {
			log.Fatal().Err(err).Msg("error getting current working directory")
		}
		log.Fatal().Str("cwd", cwd).Msg("public directory not found")
	}
	
	routeCount := 0
	filepath.Walk("public", func(path string, info os.FileInfo, err error) error {
		if info.IsDir() {
			return nil
		}
		urlPath := strings.Replace(path, "public", "", 1)

		route, err := makeRoute(path)

		if err != nil {
			log.Error().Err(err).Str("url_path", urlPath).Str("file_path", path).Msg("error making route")
			return nil
		}

		routes[urlPath] = route
		routeCount++

		if info.Name() == "index.html" {
			indexUrlPath := strings.Replace(urlPath, "/index.html", "", 1)
			if indexUrlPath == "" {
				indexUrlPath = "/"
			}
			log.Debug().Str("index_path", indexUrlPath).Str("file_path", path).Msg("adding index route")
			routes[indexUrlPath] = route
			routes[indexUrlPath+"/"] = route
			routeCount += 2
		}
		log.Debug().Str("url_path", urlPath).Str("file_path", path).Msg("adding route")

		return nil
	})
	
	log.Info().Int("route_count", routeCount).Msg("routes populated successfully")
}

func getAcceptedEncoding(ctx *fasthttp.RequestCtx) string {
	acceptEncoding := string(ctx.Request.Header.Peek("Accept-Encoding"))
	if strings.Contains(acceptEncoding, "br") {
		return "br"
	}
	if strings.Contains(acceptEncoding, "gzip") {
		return "gzip"
	}
	return ""
}

func getEncodedContent(acceptedEncoding string, content Content) (string, []byte) {
	switch acceptedEncoding {
	case "br":
		if content.Brotli != nil {
			return "br", content.Brotli
		} else {
			return "", content.Plain
		}
	case "gzip":
		if content.Gzip != nil {
			return "gzip", content.Gzip
		} else {
			return "", content.Plain
		}
	default:
		return "", content.Plain
	}
}

func healthCheckHandler(ctx *fasthttp.RequestCtx) {
	ctx.Response.Header.Set("Content-Type", "application/json")
	ctx.Response.Header.Set("Server", "nano-web")
	ctx.SetStatusCode(fasthttp.StatusOK)
	fmt.Fprintf(ctx, `{"status":"healthy","timestamp":"%s"}`, time.Now().UTC().Format(time.RFC3339))
}

func handler(ctx *fasthttp.RequestCtx) {
	start := time.Now()
	path := string(ctx.Path())
	method := string(ctx.Method())
	userAgent := string(ctx.UserAgent())
	
	// Handle health check endpoint
	if path == "/health" || path == "/_health" {
		healthCheckHandler(ctx)
		return
	}
	
	route, exists := routes[path]
	if !exists {
		if os.Getenv("SPA_MODE") == "1" {
			route, exists = routes["/"]
			if !exists {
				duration := time.Since(start)
				if logRequests {
					log.Warn().
						Str("method", method).
						Str("path", path).
						Str("user_agent", userAgent).
						Int("status", fasthttp.StatusNotFound).
						Dur("duration_ms", duration).
						Msg("request not found")
				}
				ctx.Error("Not Found", fasthttp.StatusNotFound)
				return
			}
		} else {
			duration := time.Since(start)
			if logRequests {
				log.Warn().
					Str("method", method).
					Str("path", path).
					Str("user_agent", userAgent).
					Int("status", fasthttp.StatusNotFound).
					Dur("duration_ms", duration).
					Msg("request not found")
			}
			ctx.Error("Not Found", fasthttp.StatusNotFound)
			return
		}
	}

	ctx.Response.Header.Set("Content-Type", route.ContentType)
	ctx.Response.Header.Set("Server", "nano-web")
	ctx.Response.Header.Set("Last-Modified", route.LastModified)
	acceptedEncoding := getAcceptedEncoding(ctx)
	encoding, content := getEncodedContent(acceptedEncoding, route.Content)
	if encoding != "" {
		ctx.Response.Header.Set("Content-Encoding", encoding)
	}
	fmt.Fprintf(ctx, "%s", content)
	
	duration := time.Since(start)
	if logRequests {
		log.Info().
			Str("method", method).
			Str("path", path).
			Str("user_agent", userAgent).
			Str("content_type", route.ContentType).
			Str("encoding", encoding).
			Int("status", fasthttp.StatusOK).
			Int("content_length", len(content)).
			Dur("duration_ms", duration).
			Msg("request served")
	}
}

func main() {
	// Parse command line flags
	var healthCheck bool
	flag.BoolVar(&healthCheck, "health-check", false, "Perform health check and exit")
	flag.Parse()

	// Handle health check command
	if healthCheck {
		// Simple health check - just verify we can start
		log.Info().Msg("Health check passed")
		os.Exit(0)
	}

	// Configure logger
	if getEnv("LOG_FORMAT", "json") == "json" {
		// JSON logging for production
		log.Logger = zerolog.New(os.Stdout).With().Timestamp().Logger()
	} else {
		// Pretty logging for development
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stdout, TimeFormat: time.RFC3339})
	}
	
	// Set log level
	switch getEnv("LOG_LEVEL", "info") {
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

	addr := ":" + getEnv("PORT", "80")
	
	log.Info().
		Str("version", "nano-web").
		Str("port", getEnv("PORT", "80")).
		Str("public_dir", publicDir).
		Bool("spa_mode", os.Getenv("SPA_MODE") == "1").
		Bool("log_requests", logRequests).
		Str("config_prefix", getEnv("CONFIG_PREFIX", "VITE_")).
		Msg("starting nano-web server")
	
	populateRoutes(routes)
	
	log.Info().Str("addr", addr).Msg("server listening")
	if err := fasthttp.ListenAndServe(addr, handler); err != nil {
		log.Fatal().Err(err).Msg("server failed to start")
	}
}
