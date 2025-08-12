# Build stage
FROM rust:1-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

# Add musl target for current architecture
RUN rustup target add $(uname -m)-unknown-linux-musl

# Set working directory
WORKDIR /build

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY VERSION ./

# Build with static linking and additional optimizations for scratch image
ENV RUSTFLAGS="-C target-feature=+crt-static -C target-cpu=generic"
RUN TARGET=$(uname -m)-unknown-linux-musl && \
    cargo build --release --target $TARGET && \
    cp target/$TARGET/release/nano-web /tmp/nano-web

# Runtime stage
FROM scratch

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
