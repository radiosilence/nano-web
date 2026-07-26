# Selects which stage supplies the binary. Must be declared before the first
# FROM to be usable in one. `docker build .` compiles from source; CI passes
# prebuilt to reuse the binary the release matrix already built.
ARG BIN_SOURCE=source

# Source build.
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

# Build with static linking and additional optimizations for scratch image
ENV RUSTFLAGS="-C target-feature=+crt-static -C target-cpu=generic"
RUN TARGET=$(uname -m)-unknown-linux-musl && \
    cargo build --release --target $TARGET && \
    cp target/$TARGET/release/nano-web /tmp/nano-web

FROM scratch AS bin-source
COPY --from=builder /tmp/nano-web /nano-web

FROM scratch AS bin-prebuilt
ARG TARGETARCH
COPY dist/nano-web-linux-${TARGETARCH}-musl /nano-web

# Runtime stage. BuildKit only builds the stage this resolves to, so the
# source build is skipped entirely when BIN_SOURCE=prebuilt.
FROM bin-${BIN_SOURCE}

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

# Set as JSON logs by default
ENV LOG_FORMAT=json

# Run the server
ENTRYPOINT ["/nano-web"]
CMD ["serve", "/public"]
