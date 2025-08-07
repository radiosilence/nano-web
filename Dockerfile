# Build stage
FROM rust:1.84-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc ca-certificates

# Set working directory
WORKDIR /build

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY VERSION ./

# Add target and build the binary with optimizations
RUN rustup target add x86_64-unknown-linux-musl
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/nano-web /nano-web

# Create volume for static files
VOLUME ["/public"]

# Expose port
EXPOSE 3000

# Set labels
LABEL org.opencontainers.image.title="nano-web"
LABEL org.opencontainers.image.description="Static file server built with Rust"
LABEL org.opencontainers.image.vendor="James Cleveland"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/radiosilence/nano-web"

# Run the server
ENTRYPOINT ["/nano-web"]
CMD ["serve", "/public"]
