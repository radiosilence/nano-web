package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/alecthomas/kong"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

var appEnv map[string]string

type CLI struct {
	Serve       ServeCmd       `cmd:"" help:"Start the web server"`
	HealthCheck HealthCheckCmd `cmd:"" help:"Run health check"`
	Version     VersionCmd     `cmd:"" help:"Show version information"`
}

type ServeCmd struct {
	PublicDir    string `arg:"" help:"Directory to serve" default:"public"`
	Port         int    `short:"p" help:"Port to listen on" default:"80"`
	SpaMode      bool   `help:"Enable SPA mode (serve index.html for all routes)" default:"false"`
	ConfigPrefix string `help:"Environment variable prefix for config injection" default:"VITE_"`
	LogLevel     string `help:"Log level (debug, info, warn, error)" default:"info" enum:"debug,info,warn,error"`
	LogFormat    string `help:"Log format (json, console)" default:"json" enum:"json,console"`
	LogRequests  bool   `help:"Log HTTP requests" default:"true"`
}

type HealthCheckCmd struct{}

func (h *HealthCheckCmd) Run() error {
	// Simple health check - could be expanded to check dependencies
	fmt.Println("Health check: OK")
	return nil
}

type VersionCmd struct{}

func (v *VersionCmd) Run() error {
	fmt.Println(FullVersion())
	fmt.Println("Ultra-fast static file server built with Go")
	fmt.Println("Repository: https://github.com/radiosilence/nano-web")
	return nil
}

func (s *ServeCmd) Run() error {
	// Setup logging
	setupLogging(s.LogLevel, s.LogFormat)

	// Get app environment variables
	appEnv = getAppEnv(s.ConfigPrefix)

	// Populate routes
	err := populateRoutes(s.PublicDir)
	if err != nil {
		log.Fatal().Err(err).Msg("failed to populate routes")
		return err
	}

	// Start server
	addr := ":" + strconv.Itoa(s.Port)
	return startServer(addr, s)
}

func getAppEnv(prefix string) map[string]string {
	env := make(map[string]string)
	for _, e := range os.Environ() {
		if pair := strings.SplitN(e, "=", 2); len(pair) == 2 {
			key := pair[0]
			if strings.HasPrefix(key, prefix) {
				env[strings.TrimPrefix(key, prefix)] = pair[1]
			}
		}
	}
	return env
}

func setupLogging(level, format string) {
	// Set log level
	switch level {
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

	// Set log format
	if format == "console" {
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})
	}
}

func main() {
	cli := &CLI{}
	ctx := kong.Parse(cli)
	err := ctx.Run()
	ctx.FatalIfErrorf(err)
}
