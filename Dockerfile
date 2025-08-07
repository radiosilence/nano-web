# Build stage
FROM rust:1.84-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc ca-certificates tzdata

# Create appuser for security
RUN adduser -D -g '' appuser

# Set working directory
WORKDIR /build

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY VERSION ./

# Build the binary with maximum optimizations
ENV RUSTFLAGS="-C target-cpu=native -C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage
FROM scratch

# Copy the binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/nano-web /nano-web

# Set default environment variables
ENV PORT=3000
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
