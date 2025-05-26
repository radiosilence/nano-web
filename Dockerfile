# Build stage
FROM golang:1.24-alpine AS builder

# Install build dependencies
RUN apk add --no-cache git ca-certificates tzdata

# Create appuser for security
RUN adduser -D -g '' appuser

# Set working directory
WORKDIR /build

# Copy go mod files first for better caching
COPY go.mod go.sum ./

# Download dependencies
RUN go mod download && go mod verify

# Copy source code
COPY main.go ./

# Build the binary with optimizations
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build \
    -ldflags='-w -s -extldflags "-static"' \
    -a -installsuffix cgo \
    -o nano-web main.go

# Runtime stage
FROM scratch

# Copy the binary
COPY --from=builder /build/nano-web /nano-web

# Set default environment variables
ENV PORT=80
ENV SPA_MODE=0
ENV LOG_LEVEL=info
ENV LOG_FORMAT=json
ENV LOG_REQUESTS=true
ENV CONFIG_PREFIX=VITE_

# Expose port
EXPOSE $PORT

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/nano-web", "--health-check"] || exit 1

# Set labels for better maintainability
LABEL org.opencontainers.image.title="nano-web"
LABEL org.opencontainers.image.description="Hyper-minimal, lightning-fast web server for SPAs and static content"
LABEL org.opencontainers.image.vendor="nano-web"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/radiosilence/nano-web"

# Run the binary
ENTRYPOINT ["/nano-web", "serve", "public"]
