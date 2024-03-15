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
	"text/template"

	"github.com/andybalholm/brotli"
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
			fmt.Println("⇨ error getting current working directory", err)
			os.Exit(-1)
		}
		fmt.Println("⇨ public directory not found in: " + cwd)
		os.Exit(-1)
	}
	filepath.Walk("public", func(path string, info os.FileInfo, err error) error {
		if info.IsDir() {
			return nil
		}
		urlPath := strings.Replace(path, "public", "", 1)

		route, err := makeRoute(path)

		if err != nil {
			fmt.Errorf("⇨ error making route for %s: %s", urlPath, err)
			return nil
		}

		routes[urlPath] = route

		if info.Name() == "index.html" {
			indexUrlPath := strings.Replace(urlPath, "/index.html", "", 1)
			if indexUrlPath == "" {
				indexUrlPath = "/"
			}
			fmt.Println("⇨ adding index", indexUrlPath, "→", path)
			routes[indexUrlPath] = route
		}
		fmt.Println("⇨ adding route", urlPath, "→", path)

		return nil
	})
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

func handler(ctx *fasthttp.RequestCtx) {
	fmt.Println("⇨ request", string(ctx.Path()))
	route, exists := routes[string(ctx.Path())]
	if !exists {
		if os.Getenv("SPA_MODE") == "1" {
			route, exists = routes["/"]
			if !exists {
				ctx.Error("Not Found", fasthttp.StatusNotFound)
				return
			}
		} else {
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
}

func main() {
	addr := ":" + getEnv("PORT", "80")
	populateRoutes(routes)
	// fmt.Printf("⇨ routes:\n")
	// pp.Print(routes)
	fasthttp.ListenAndServe(addr, handler)
}
