# CI builds the static musl binary; this image only copies it in. Building it
# by hand needs dist/ populated first — docker builds only ever happen in CI.
#
# No CA bundle here, unlike the MCP images: nano-web serves files and makes no
# outbound TLS connections, so it needs no trust store.
FROM scratch

ARG TARGETARCH

COPY dist/nano-web-linux-${TARGETARCH}-musl /nano-web

VOLUME ["/public"]
EXPOSE 3000

LABEL org.opencontainers.image.title="nano-web"
LABEL org.opencontainers.image.description="Static file server built with Rust"
LABEL org.opencontainers.image.vendor="James Cleveland"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/radiosilence/nano-web"

ENV LOG_FORMAT=json

ENTRYPOINT ["/nano-web"]
CMD ["serve", "/public"]
