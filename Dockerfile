# Build stage
FROM rust:1.84-alpine AS builder

# Get target architecture
ARG TARGETARCH

# Install build dependencies
RUN apk add --no-cache musl-dev gcc ca-certificates

# Install cross-compilation tools for ARM64
RUN if [ "$TARGETARCH" = "arm64" ]; then \
    apk add --no-cache aarch64-linux-musl-cross; \
fi

# Set working directory
WORKDIR /build

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY VERSION ./

# Set Rust target based on architecture
RUN if [ "$TARGETARCH" = "amd64" ]; then \
        export RUST_TARGET="x86_64-unknown-linux-musl"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        export RUST_TARGET="aarch64-unknown-linux-musl"; \
    fi && \
    rustup target add $RUST_TARGET && \
    echo "RUST_TARGET=$RUST_TARGET" > /tmp/rust_target

# Build the binary with optimizations
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN export RUST_TARGET=$(cat /tmp/rust_target | cut -d= -f2) && \
    if [ "$TARGETARCH" = "arm64" ]; then \
        export CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc; \
        export AR_aarch64_unknown_linux_musl=aarch64-linux-musl-ar; \
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc; \
    fi && \
    cargo build --release --target $RUST_TARGET && \
    cp target/$RUST_TARGET/release/nano-web /tmp/nano-web

# Runtime stage
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the binary
COPY --from=builder /tmp/nano-web /nano-web

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
