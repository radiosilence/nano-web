package main

import (
	"io"
	"net/http"
	"os"
	"text/template"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

type Template struct {
	templates *template.Template
}

func (t *Template) Render(w io.Writer, name string, data interface{}, c echo.Context) error {
	return t.templates.ExecuteTemplate(w, name, data)
}

func Hello(c echo.Context) error {
	return c.Render(http.StatusOK, "hello", "World")
}
func main() {
	e := echo.New()
	port, exists := os.LookupEnv("PORT")

	if !exists {
		port = "80"
	}

	addr := ":" + port

	if os.Getenv("SPA_MODE") == "1" {
		e.Static("/assets", "public/assets")

		t := &Template{
			templates: template.Must(template.ParseGlob("public/*.html")),
		}
		e.Renderer = t

		e.GET("/*", Hello)
	} else {
		e.Static("/", "public")
	}

	e.Use(middleware.Gzip())
	e.Use(middleware.Logger())

	e.Logger.Fatal(e.Start(addr))
}
