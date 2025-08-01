package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

const (
	DefaultPort = 3000
)

var appEnv map[string]string

var (
	publicDir    string
	port         int
	dev          bool
	spaMode      bool
	configPrefix string
	logLevel     string
	logFormat    string
	logRequests  bool
)

var rootCmd = &cobra.Command{
	Use:   "nano-web [directory]",
	Short: "Ultra-fast static file server built with Go",
	Long:  "🔥 Ultra-fast static file server built with Go\nRepository: https://github.com/radiosilence/nano-web",
	Args:  cobra.MaximumNArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if len(args) > 0 {
			publicDir = args[0]
		}
		
		// Setup logging
		setupLogging(logLevel, logFormat)

		// Get app environment variables
		appEnv = getAppEnv(configPrefix)

		// Populate routes
		err := populateRoutes(publicDir)
		if err != nil {
			log.Fatal().Err(err).Msg("failed to populate routes")
			return err
		}

		// Start server
		addr := ":" + strconv.Itoa(port)
		return startServer(addr, &ServeConfig{
			PublicDir:    publicDir,
			Port:         port,
			Dev:          dev,
			SpaMode:      spaMode,
			ConfigPrefix: configPrefix,
			LogLevel:     logLevel,
			LogFormat:    logFormat,
			LogRequests:  logRequests,
		})
	},
}

var serveCmd = &cobra.Command{
	Use:   "serve [directory]",
	Short: "Start the web server",
	Long:  "Start the web server to serve static files from the specified directory",
	Args:  cobra.MaximumNArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if len(args) > 0 {
			publicDir = args[0]
		}
		
		// Setup logging
		setupLogging(logLevel, logFormat)

		// Get app environment variables
		appEnv = getAppEnv(configPrefix)

		// Populate routes
		err := populateRoutes(publicDir)
		if err != nil {
			log.Fatal().Err(err).Msg("failed to populate routes")
			return err
		}

		// Start server
		addr := ":" + strconv.Itoa(port)
		return startServer(addr, &ServeConfig{
			PublicDir:    publicDir,
			Port:         port,
			Dev:          dev,
			SpaMode:      spaMode,
			ConfigPrefix: configPrefix,
			LogLevel:     logLevel,
			LogFormat:    logFormat,
			LogRequests:  logRequests,
		})
	},
}

var versionCmd = &cobra.Command{
	Use:   "version",
	Short: "Show version information",
	RunE: func(cmd *cobra.Command, args []string) error {
		fmt.Println(FullVersion())
		fmt.Println("🔥 Ultra-fast static file server built with Go")
		fmt.Println("Repository: https://github.com/radiosilence/nano-web")
		return nil
	},
}

var completionCmd = &cobra.Command{
	Use:                   "completion [bash|zsh|fish|powershell]",
	Short:                 "Generate completion script",
	DisableFlagsInUseLine: true,
	ValidArgs:             []string{"bash", "zsh", "fish", "powershell"},
	Args:                  cobra.MatchAll(cobra.ExactArgs(1), cobra.OnlyValidArgs),
	RunE: func(cmd *cobra.Command, args []string) error {
		switch args[0] {
		case "bash":
			return rootCmd.GenBashCompletion(os.Stdout)
		case "zsh":
			return rootCmd.GenZshCompletion(os.Stdout)
		case "fish":
			return rootCmd.GenFishCompletion(os.Stdout, true)
		case "powershell":
			return rootCmd.GenPowerShellCompletionWithDesc(os.Stdout)
		}
		return nil
	},
}

type ServeConfig struct {
	PublicDir    string
	Port         int
	Dev          bool
	SpaMode      bool
	ConfigPrefix string
	LogLevel     string
	LogFormat    string
	LogRequests  bool
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

func init() {
	rootCmd.AddCommand(serveCmd)
	rootCmd.AddCommand(versionCmd)
	rootCmd.AddCommand(completionCmd)

	// Root command flags (also applies to serve command for backward compatibility)
	rootCmd.Flags().StringVarP(&publicDir, "dir", "", getEnvOrDefault("PUBLIC_DIR", "public"), "Directory to serve")
	rootCmd.Flags().IntVarP(&port, "port", "p", getEnvIntOrDefault("PORT", DefaultPort), "Port to listen on")
	rootCmd.Flags().BoolVarP(&dev, "dev", "d", getEnvBoolOrDefault("DEV", false), "Check/reload files if modified")
	rootCmd.Flags().BoolVar(&spaMode, "spa", getEnvBoolOrDefault("SPA_MODE", false), "Enable SPA mode (serve index.html for all routes)")
	rootCmd.Flags().StringVar(&configPrefix, "config-prefix", getEnvOrDefault("CONFIG_PREFIX", "VITE_"), "Environment variable prefix for config injection")
	rootCmd.Flags().StringVar(&logLevel, "log-level", getEnvOrDefault("LOG_LEVEL", "info"), "Log level (debug, info, warn, error)")
	rootCmd.Flags().StringVar(&logFormat, "log-format", getEnvOrDefault("LOG_FORMAT", "console"), "Log format (json, console)")
	rootCmd.Flags().BoolVar(&logRequests, "log-requests", getEnvBoolOrDefault("LOG_REQUESTS", true), "Log HTTP requests")

	// Serve command flags (inherits from root)
	serveCmd.Flags().AddFlagSet(rootCmd.Flags())

	// Add completion for log level
	rootCmd.RegisterFlagCompletionFunc("log-level", func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		return []string{"debug", "info", "warn", "error"}, cobra.ShellCompDirectiveDefault
	})

	// Add completion for log format
	rootCmd.RegisterFlagCompletionFunc("log-format", func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		return []string{"json", "console"}, cobra.ShellCompDirectiveDefault
	})

	// Add directory completion for the root command argument
	rootCmd.ValidArgsFunction = func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		return nil, cobra.ShellCompDirectiveFilterDirs
	}

	// Add directory completion for the serve command argument
	serveCmd.ValidArgsFunction = func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		return nil, cobra.ShellCompDirectiveFilterDirs
	}
}

func getEnvOrDefault(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}

func getEnvIntOrDefault(key string, defaultValue int) int {
	if value := os.Getenv(key); value != "" {
		if intValue, err := strconv.Atoi(value); err == nil {
			return intValue
		}
	}
	return defaultValue
}

func getEnvBoolOrDefault(key string, defaultValue bool) bool {
	if value := os.Getenv(key); value != "" {
		if boolValue, err := strconv.ParseBool(value); err == nil {
			return boolValue
		}
	}
	return defaultValue
}

func main() {
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}
