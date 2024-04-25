import * as fs from "fs";
import * as path from "path";
import * as process from "process";
import * as zlib from "zlib";

interface Content {
  plain: Uint8Array;
  gzip?: Uint8Array;
  brotli?: Uint8Array;
}

interface Route {
  content: Content;
  contentType: string;
  lastModified: string;
}

interface Routes {
  [path: string]: Route;
}

function getEnv(name: string, fallback: string) {
  return process.env[name] ?? fallback;
}

function getAppEnv() {
  const prefix: string = getEnv("CONFIG_PREFIX", "VITE_");
  const appEnv: { [key: string]: string } = {};
  for (const [key, value] of Object.entries(process.env)) {
    if (key.startsWith(prefix) && value !== undefined) {
      appEnv[key.replace(prefix, "")] = value;
    }
  }
  return appEnv;
}

const appEnv = getAppEnv();
const publicDir = getEnv("PUBLIC_DIR", "public");

function getMimeType(ext: string): string {
  switch (ext) {
    case ".html":
      return "text/html";
    case ".css":
      return "text/css";
    case ".js":
      return "text/javascript";
    case ".json":
      return "application/json";
    case ".xml":
      return "application/xml";
    case ".pdf":
      return "application/pdf";
    case ".zip":
      return "application/zip";
    case ".doc":
      return "application/msword";
    case ".eot":
      return "application/vnd.ms-fontobject";
    case ".otf":
      return "font/otf";
    case ".ttf":
      return "font/ttf";
    case ".woff":
      return "font/woff";
    case ".woff2":
      return "font/woff2";
    case ".gif":
      return "image/gif";
    case ".jpeg":
    case ".jpg":
      return "image/jpeg";
    case ".png":
      return "image/png";
    case ".svg":
      return "image/svg+xml";
    case ".ico":
      return "image/x-icon";
    case ".webp":
      return "image/webp";
    case ".mp4":
      return "video/mp4";
    case ".webm":
      return "video/webm";
    case ".wav":
      return "audio/wav";
    case ".mp3":
      return "audio/mpeg";
    case ".ogg":
      return "audio/ogg";
    case ".csv":
      return "text/csv";
    case ".txt":
      return "text/plain";
    default:
      return "application/octet-stream";
  }
}

interface TemplateData {
  env: { [key: string]: string };
  json: string;
  escapedJson: string;
}

async function templateRoute(name: string, content: string): Promise<string> {
  // const tmpl = new template.Template(name);
  // const jsonString = JSON.stringify(appEnv);
  // const tmplData: TemplateData = {
  //   env: appEnv,
  //   json: jsonString,
  //   escapedJson: jsonString.replace(/"/g, '\\"'),
  // };
  // await tmpl.parse(content);
  // return tmpl.render(tmplData);
  return content;
}

function templateType(mimeType: string) {
  switch (mimeType) {
    case "text/html":
    case "text/css":
    case "text/javascript":
    case "application/json":
      return true;
    default:
      return false;
  }
}

function compressedType(mimeType: string) {
  switch (mimeType) {
    case "text/html":
    case "text/css":
    case "text/javascript":
    case "application/json":
      return true;
    default:
      return false;
  }
}

function gzipData(data: Uint8Array) {
  return zlib.gzipSync(data);
}

function brotliData(data: Uint8Array) {
  return zlib.brotliCompressSync(data);
}

async function makeRoute(filePath: string) {
  const ext = path.extname(filePath).toLowerCase();
  const mimeType = getMimeType(ext);
  const plain = await fs.promises.readFile(filePath);
  const stats = await fs.promises.stat(filePath);

  let content: Content = { plain };

  if (templateType(mimeType)) {
    const templatedContent = await templateRoute(filePath, plain.toString());
    content = { plain: Buffer.from(templatedContent) };
  }

  if (compressedType(mimeType)) {
    try {
      content.gzip = gzipData(plain);
    } catch (err) {
      console.warn(`⇨ gzip compression failed for ${filePath}: ${err}`);
    }
    try {
      content.brotli = brotliData(plain);
    } catch (err) {
      console.warn(`⇨ brotli compression failed for ${filePath}: ${err}`);
    }
  }

  return {
    content,
    contentType: mimeType,
    lastModified: stats.mtime.toUTCString(),
  };
}

async function populateRoutes() {
  const routes: Routes = {};
  try {
    await fs.promises.stat(publicDir);
    const files = await fs.promises.readdir(publicDir);
    for (const filename of files) {
      const filePath = path.join(publicDir, filename);
      try {
        const stats = await fs.promises.stat(filePath);
        if (stats.isFile()) {
          const urlPath = path.relative(publicDir, filePath);
          try {
            const route = await makeRoute(filePath);
            routes[urlPath] = route;
            if (filename === "index.html") {
              let indexUrlPath = urlPath.replace("index.html", "");
              console.log("urlPath", urlPath);
              console.log("indexUrlPath", indexUrlPath);
              if (indexUrlPath === "") {
                indexUrlPath = "/";
              } else {
                routes[indexUrlPath + "/"] = route;
              }
              routes[indexUrlPath] = route;
            }
            console.log("⇨ adding route", urlPath, "→", filePath);
          } catch (err) {
            console.error(`⇨ error making route for ${urlPath}: ${err}`);
          }
        }
      } catch (err) {
        console.error(`⇨ error reading file ${filePath}: ${err}`);
      }
    }
  } catch (err) {
    console.error(`⇨ public directory not found in: ${publicDir}`);
    process.exit(-1);
  }
  return routes;
}

function getAcceptedEncodings(req: Request) {
  const accepted = [];
  const acceptEncoding = req.headers.get("Accept-Encoding") || "";
  if (acceptEncoding.includes("br")) {
    accepted.push("br");
  }
  if (acceptEncoding.includes("gzip")) {
    accepted.push("gzip");
  }
  return accepted;
}

function getEncodedContent(
  acceptedEncodings: string[],
  content: Content
): [string, Uint8Array] {
  if (acceptedEncodings.includes("br") && content.brotli) {
    return ["br", content.brotli];
  }
  if (acceptedEncodings.includes("gzip") && content.gzip) {
    return ["gzip", content.gzip];
  }
  return ["", content.plain];
}

const routes = await populateRoutes();

for (const [path, route] of Object.entries(routes)) {
  console.log("⇨ serving route", path, route.contentType);
}
Bun.serve({
  async fetch(req) {
    const path = new URL(req.url).pathname;
    console.log("⇨ request", path);
    let route = routes[path];
    if (!route) {
      if (process.env.SPA_MODE === "1") {
        const indexRoute = routes["/"];
        if (indexRoute) {
          route = indexRoute;
        }
      }
      if (!route) {
        return new Response("Not Found", { status: 404 });
      }
    }

    const headers: Record<string, string> = {
      "Content-Type": route.contentType,
      Server: "nano-web",
      "Last-Modified": route.lastModified,
    };

    const acceptedEncodings = getAcceptedEncodings(req);
    const [encoding, content] = getEncodedContent(
      acceptedEncodings,
      route.content
    );
    if (encoding) {
      headers["Content-Encoding"] = encoding;
    }
    return new Response(content, { headers });
  },
});
