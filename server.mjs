import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const PORT = Number(process.env.PORT || 3000);
const ROOT = process.cwd();
const WEBSITE_DIR = path.join(ROOT, "website");
const DOCS_DIR = path.join(ROOT, "docs");
const EXAMPLES_DIR = path.join(ROOT, "examples");

const ASSET_FILES = {
  "/assets/dog_hero_workbench.jpg": path.join(
    ROOT,
    "src/assets/images/dog_hero_workbench_1795424631983.jpg"
  ),
  "/assets/dog_emblem_badge.jpg": path.join(
    ROOT,
    "src/assets/images/dog_emblem_badge_1795424632430.jpg"
  ),
};

const MIME_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".mjs": "application/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

function collectFiles(dir, prefix = "") {
  const results = [];
  if (!fs.existsSync(dir)) return results;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "target" || entry.name === "node_modules") {
      continue;
    }
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) {
      results.push(...collectFiles(path.join(dir, entry.name), rel));
    } else {
      results.push(rel);
    }
  }
  return results.sort();
}

function sendJson(res, status, payload) {
  res.writeHead(status, { "Content-Type": "application/json; charset=utf-8" });
  res.end(JSON.stringify(payload, null, 2));
}

function sendFile(res, filePath) {
  if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
    res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
    res.end("Not Found\n");
    return;
  }
  const ext = path.extname(filePath).toLowerCase();
  const mime = MIME_TYPES[ext] || "application/octet-stream";
  res.writeHead(200, { "Content-Type": mime });
  fs.createReadStream(filePath).pipe(res);
}

function listDirSafe(dir, extFilter) {
  if (!fs.existsSync(dir)) return [];
  const exts = Array.isArray(extFilter) ? extFilter : extFilter ? [extFilter] : null;
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && !e.name.startsWith(".") && (!exts || exts.some((ext) => e.name.endsWith(ext))))
    .map((e) => e.name)
    .sort();
}

const server = http.createServer((req, res) => {
  const parsed = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
  const pathname = decodeURIComponent(parsed.pathname);

  if (pathname === "/health" || pathname === "/api/files") {
    return sendJson(res, 200, {
      status: "ok",
      language: "Rust (dog-lang)",
      version: "0.3.0",
      files: collectFiles(ROOT),
    });
  }

  if (pathname === "/api/workspace") {
    return sendJson(res, 200, {
      version: "0.3.0",
      docs: listDirSafe(DOCS_DIR, ".md"),
      examples: listDirSafe(EXAMPLES_DIR, [".dg", ".dgss"]),
    });
  }

  if (pathname === "/api/file") {
    const kind = parsed.searchParams.get("kind");
    const name = parsed.searchParams.get("name") || "";
    if (!name || name.includes("..") || name.includes("/") || name.includes("\\")) {
      return sendJson(res, 400, { error: "Invalid file name." });
    }
    const baseDir = kind === "doc" ? DOCS_DIR : kind === "example" ? EXAMPLES_DIR : null;
    if (!baseDir) {
      return sendJson(res, 400, { error: "Invalid kind; expected 'doc' or 'example'." });
    }
    const targetPath = path.resolve(baseDir, name);
    if (!targetPath.startsWith(baseDir + path.sep) || !fs.existsSync(targetPath)) {
      return sendJson(res, 404, { error: "File not found." });
    }
    const content = fs.readFileSync(targetPath, "utf8");
    return sendJson(res, 200, { kind, name, content });
  }

  if (Object.prototype.hasOwnProperty.call(ASSET_FILES, pathname)) {
    return sendFile(res, ASSET_FILES[pathname]);
  }

  const relPath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
  const resolved = path.resolve(WEBSITE_DIR, relPath);
  if (!resolved.startsWith(WEBSITE_DIR + path.sep) && resolved !== WEBSITE_DIR) {
    res.writeHead(403, { "Content-Type": "text/plain; charset=utf-8" });
    res.end("Forbidden\n");
    return;
  }

  if (fs.existsSync(resolved) && fs.statSync(resolved).isFile()) {
    return sendFile(res, resolved);
  }

  return sendFile(res, path.join(WEBSITE_DIR, "index.html"));
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`DOG official website server listening on http://0.0.0.0:${PORT}`);
});
