# Build stage
FROM golang:1.24-alpine AS builder

# Install build dependencies and Task
RUN apk add --no-cache git ca-certificates tzdata curl

# Install task
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d

# Create appuser for security
RUN adduser -D -g '' appuser

# Set working directory
WORKDIR /build

# Copy go mod files and Taskfile first for better caching
COPY .git go.mod go.sum Taskfile.yml ./

# Download dependencies using Task
RUN task deps

# Copy source code and VERSION file
COPY *.go .
COPY VERSION ./

# Build the binary using Task
ENV CGO_ENABLED=0
ENV GOOS=linux
ENV GOARCH=amd64
RUN task build

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
    CMD curl -f http://localhost:$PORT/_health || exit 1

# Set labels for better maintainability
LABEL org.opencontainers.image.title="nano-web"
LABEL org.opencontainers.image.description="Static file server for SPAs and static content"
LABEL org.opencontainers.image.vendor="nano-web"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/radiosilence/nano-web"

# Run the binary
ENTRYPOINT ["/nano-web", "serve", "--dir", "/public"]
