package main

import (
	_ "embed"
	"strings"
)

//go:embed VERSION
var versionFile string

// Version returns the current version of nano-web
func Version() string {
	return strings.TrimSpace(versionFile)
}

// FullVersion returns the version with "nano-web v" prefix
func FullVersion() string {
	return "nano-web v" + Version()
}
