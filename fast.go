package fast

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type Route struct {
	Content      bytes.Buffer
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
var routes Routes

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
	default:
		return "application/octet-stream"
	}
}

func makeRoute(filepath string) Route {
	ext := strings.ToLower(filepath[strings.LastIndex(filepath, "."):])
	mimetype := getMimetype(ext)
	if isTemplateableType(mimetype) {
	}
	return Route{
		Content:      bytes.Buffer{}, // TODO
		ContentType:  mimetype,
		LastModified: "TODO", // TODO
	}
}

func isTemplateableType(mimetype string) bool {
	switch mimetype {
	case "text/html", "text/css", "text/javascript", "application/json":
		return true
	default:
		return false
	}
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
		if info.Name() == "index.html" {
			indexUrlPath := strings.Replace(urlPath, "/index.html", "", 1)
			if indexUrlPath == "" {
				indexUrlPath = "/"
			}
			fmt.Println("⇨ adding index", indexUrlPath, "→", path)
			routes[indexUrlPath] = makeRoute(path)
		}
		fmt.Println("⇨ adding route", urlPath, "→", path)
		routes[urlPath] = makeRoute(path)
		return nil
	})
}

func main() {
	populateRoutes(routes)
}
