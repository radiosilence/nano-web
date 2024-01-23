# nano-web

Hyper-minimal single binary gzipping webserver for serving static content based on labstack echo-server. Based on alpine.

Has SPA mode which includes runtime templating of vite environment variables.

Serves from `/public`

# Config as ENV

- `PORT` The port to listen on. Defaults to `80`
- `SPA_MODE` when set to `1` 404 request will return `/public/index.html` as a `200`.

# Example Dockerfile

```Dockerfile

FROM ghcr.io/compliance-framework/nano-web:latest
COPY ./dist /public/
ENV PORT=8081
ENV SPA_MODE=1

```
