# nano-web

Hyper-minimal single binary gzipping webserver for serving static content based on labstack echo-server. Based on alpine.

Serves from `/public`

# Config as ENV

- `PORT` The port to listen on
- `SPA_MODE` when set to `1` it only serves up ` /public/assets`` and all other requests go to  `/public/index.html`
