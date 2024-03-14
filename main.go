package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"text/template"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

type Template struct {
	templates *template.Template
}

type TemplateData struct {
	Env         map[string]string `json:"env"`
	Json        string            `json:"json"`
	EscapedJson string            `json:"escapedJson"`
}

func (t *Template) Render(w io.Writer, name string, data any, c echo.Context) error {
	fmt.Printf("%+v\n", data)
	return t.templates.ExecuteTemplate(w, name, data)
}

func getAppEnv() map[string]string {
	prefix := getEnv("CONFIG_PREFIX", "VITE_")
	data := make(map[string]string)
	for _, env := range os.Environ() {
		parts := strings.Split(env, "=")
		key := parts[0]
		value := strings.Join(parts[1:], "=")
		if strings.HasPrefix(key, prefix) {
			data[strings.Replace(key, prefix, "", 1)] = value
		}
	}
	return data
}

var appEnv = getAppEnv()

func Index(c echo.Context) error {
	jsonString, err := json.Marshal(appEnv)
	if err != nil {
		return err
	}
	return c.Render(http.StatusOK, "index.html", &TemplateData{
		Env:         appEnv,
		Json:        string(jsonString),
		EscapedJson: strings.Replace(string(jsonString), "\"", "\\\"", -1),
	})
}

func getEnv(name string, fallback string) string {
	value, exists := os.LookupEnv(name)
	if !exists {
		value = fallback
	}
	return value
}

func main() {
	e := echo.New()
	addr := ":" + getEnv("PORT", "80")
	_, err := os.Stat("public")

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
			e.File(indexUrlPath, path)
		}
		fmt.Println("⇨ adding route", urlPath, "→", path)
		e.File(urlPath, path)
		return nil
	})

	if os.Getenv("SPA_MODE") == "1" {
		t := &Template{
			templates: template.Must(template.ParseFiles("public/index.html")),
		}
		e.Renderer = t
		e.GET("/*", Index)
		e.GET("/", Index)
	}

	e.Use(middleware.Gzip())
	e.Use(middleware.Logger())

	e.Logger.Fatal(e.Start(addr))
}
