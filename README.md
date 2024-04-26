# nano-web

![publish-container](https://github.com/radiosilence/nano-web/actions/workflows/publish-container.yml/badge.svg) ![publish-package-amd64](https://github.com/radiosilence/nano-web/actions/workflows/publish-package-amd64.yml/badge.svg)

Hyper-minimal low-latency webserver for serving SPAs and static content based on fasthttp.

- Precaches, templates, compresses all resources into memory at startup to reduce latency.
- Brotli and gzip compression.
- Designed to work as a docker base image or as a nanovm unikernel.
- Includes runtime templating of environment variables (configurable prefix).
- Index pages so works nicely with things like Astro from the get-go.
- SPA mode to service 404s as index (200) to support client side routing.

# Config as ENV

- `PORT` The port to listen on. Defaults to `80`
- `SPA_MODE` when set to `1` 404 request will return `/public/index.html` as a `200`.
- `CONFIG_PREFIX` will set the prefix to scan environment variables in order to enable runtime config. Defaults to `VITE_`

# Docker Quick Start

```Dockerfile

FROM ghcr.io/radiosilence/nano-web:latest
COPY ./dist /public/
ENV PORT=8081
ENV SPA_MODE=1

```

# Nanos/OPS Quick Start

You'll want a `config.json` for your project that looks something like this:

```json
{
  "Dirs": ["public"],
  "Env": {
    "SPA_MODE": "1",
    "PORT": "8081"
  },
  "RunConfig": {
    "Ports": ["8081"]
  }
}
```

Make sure your public files are in a `./public` directory relative to CWD.

Then you can run this command to build your image:

```
ops image create -c config.json --package radiosilence/nano-web:latest -i my-website
```

Then run locally to test:

```
ops instance create my-website -c ./config.json --port 8081
```

# Runtime config for SPAs

**THIS IS NOT INTENDED FOR STORING SECRETS, ALL VARIABLES WILL BE PUBLIC TO CLIENT**

If are using `SPA_MODE` and you have set `CONFIG_PREFIX`, or use variables starting with `VITE_` by default, the server will
allow injection of environment variables at runtime, which is useful for configuring dynamically changing API urls, client IDs,
etc, in a dynamically scaling/routing environment such as Kubernetes.

Here's an example `index.html` that utilises this:

```html
<!DOCTYPE html>
<html lang="en" data-theme="cf">
  <head>
    <script>
      window.RUNTIME_ENV = "{{.EscapedJson}}";
    </script>
  </head>
</html>
```

And your client side TS which is safe to be bundled:

```typescript
let runtimeEnv: Record<string, string> = {};
try {
  runtimeEnv = JSON.parse((window as any).RUNTIME_ENV ?? "{}");
} catch {
  // do nothing
}
```

In this way, you can reference these variables that can be set when the container is spun-up.
