import * as fs from 'fs';
import * as http from 'http';
import * as path from 'path';
import * as process from 'process';
import * as template from 'template';
import * as zlib from 'zlib';

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

function getEnv(name: string, fallback: string): string {
    const value: string | undefined = process.env[name];
    return value !== undefined ? value : fallback;
}

function getAppEnv(): { [key: string]: string } {
    const prefix: string = getEnv("CONFIG_PREFIX", "VITE_");
    const appEnv: { [key: string]: string } = {};
    for (const env of Object.entries(process.env)) {
        const [key, value] = env;
        if (key.startsWith(prefix)) {
            appEnv[key.replace(prefix, "")] = value;
        }
    }
    return appEnv;
}

const appEnv = getAppEnv();
const publicDir = getEnv("PUBLIC_DIR", "public");
const routes: Routes = {};

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
    const tmpl = new template.Template(name);
    const jsonString = JSON.stringify(appEnv);
    const tmplData: TemplateData = {
        env: appEnv,
        json: jsonString,
        escapedJson: jsonString.replace(/"/g, "\\\""),
    };
    await tmpl.parse(content);
    return tmpl.render(tmplData);
}

function templateType(mimeType: string): boolean {
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

function compressedType(mimeType: string): boolean {
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

function gzipData(data: Uint8Array): Uint8Array {
    return zlib.gzipSync(data);
}

function brotliData(data: Uint8Array): Uint8Array {
    return zlib.brotliCompressSync(data);
}

async function makeRoute(filePath: string): Promise<Route> {
    const ext = path.extname(filePath).toLowerCase();
    const mimeType = getMimeType(ext);
    const data = await fs.promises.readFile(filePath);

    const stats = await fs.promises.stat(filePath);

    let content: Content = { plain: data };

    if (templateType(mimeType)) {
        const templatedContent = await templateRoute(filePath, data.toString());
        content = { plain: Buffer.from(templatedContent) };
    }

    if (compressedType(mimeType)) {
        content.gzip = gzipData(data);
        content.brotli = brotliData(data);
    }

    return {
        content,
        contentType: mimeType,
        lastModified: stats.mtime.toUTCString(),
    };
}

function populateRoutes(routes: Routes) {
    fs.promises.stat(publicDir)
        .then(() => {
            fs.promises.readdir(publicDir)
                .then(files => {
                    files.forEach(fileName => {
                        const filePath = path.join(publicDir, fileName);
                        fs.promises.stat(filePath)
                            .then(stats => {
                                if (stats.isFile()) {
                                    const routePath = path.relative(publicDir, filePath);
                                    makeRoute(filePath)
                                        .then(route => {
                                            routes[routePath] = route;

                                            if (fileName === "index.html") {
                                                let indexUrlPath = routePath.replace("/index.html", "");
                                                if (indexUrlPath === "") {
                                                    indexUrlPath = "/";
                                                }
                                                routes[indexUrlPath] = route;
                                                routes[indexUrlPath + "/"] = route;
                                            }
                                            console.log("⇨ adding route", routePath, "→", filePath);
                                        })
                                        .catch(err => console.error(`⇨ error making route for ${routePath}: ${err}`));
                                }
                            })
                            .catch(err => console.error(`⇨ error reading file ${filePath}: ${err}`));
                    });
                })
                .catch(err => console.error(`⇨ error reading directory ${publicDir}: ${err}`));
        })
        .catch(err => {
            console.error(`⇨ public directory not found in: ${publicDir}`);
            process.exit(-1);
        });
}
