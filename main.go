package main

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"text/template"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

type Template struct {
	templates *template.Template
}

type TemplateData struct {
	Env map[string]string `json:"env"`
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
	return c.Render(http.StatusOK, "index.html", &TemplateData{
		Env: appEnv,
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

	if os.Getenv("SPA_MODE") == "1" {
		e.Static("/assets", "public/assets")

		t := &Template{
			templates: template.Must(template.ParseFiles("public/index.html")),
		}
		e.Renderer = t

		e.GET("/*", Index)
	} else {
		e.Static("/", "public")
	}

	e.Use(middleware.Gzip())
	e.Use(middleware.Logger())

	e.Logger.Fatal(e.Start(addr))
}
