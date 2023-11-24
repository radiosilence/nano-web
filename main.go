package main

import (
	"os"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

func main() {
	e := echo.New()
	port, exists := os.LookupEnv("PORT")

	if !exists {
		port = "80"
	}

	addr := ":" + port

	// TODO: This should serve all files not just /assets
	if os.Getenv("SPA_MODE") == "1" {
		e.Static("/assets", "public/assets")
		e.File("/*", "public/index.html")
	} else {
		e.Static("/", "public")
	}

	e.Use(middleware.Gzip())
	e.Use(middleware.Logger())

	e.Logger.Fatal(e.Start(addr))
}
