import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { loadDefaultRoadmap, queryRoadmap } from "./website/roadmap_engine.mjs";
import { evaluateReadinessGate } from "./website/readiness_engine.mjs";
import {
  applyStatsEvent,
  collectCodebaseMetrics,
  fetchPublicAdoptionMetrics,
  loadStatsFile,
  saveStatsFile,
  summarizeSiteStats,
} from "./website/stats_engine.mjs";

const PORT = Number(process.env.PORT || 3000);
const ROOT = process.cwd();
const WEBSITE_DIR = path.join(ROOT, "website");
const DOCS_DIR = path.join(ROOT, "docs");
const EXAMPLES_DIR = path.join(ROOT, "examples");
const TEMPLATES_DIR = path.join(ROOT, "templates");
const STATS_FILE = path.join(ROOT, ".dog_site_stats.json");
const UPGRADE_TXT_PATH = path.join(
  ROOT,
  "public/assets/aistudio/DOG_v1.0.0_10150_Upgrade_Ideas_89683764.txt"
);

const ASSET_FILES = {
  "/assets/dog_hero_workbench.jpg": path.join(
    ROOT,
    "src/assets/images/dog_hero_workbench_1795424631983.jpg"
  ),
  "/assets/dog_emblem_badge.jpg": path.join(
    ROOT,
    "src/assets/images/dog_emblem_badge_1795424632430.jpg"
  ),
  "/assets/aistudio/DOG_v1.0.0_10150_Upgrade_Ideas_89683764.txt": UPGRADE_TXT_PATH,
};

const MIME_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".mjs": "application/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

let siteStatsState = loadStatsFile(STATS_FILE);
let roadmapCache = null;

function getRoadmapDataset() {
  if (!roadmapCache) {
    roadmapCache = loadDefaultRoadmap(ROOT);
  }
  return roadmapCache;
}

function getDogVersion() {
  try {
    const cargoRaw = fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8");
    const m = cargoRaw.match(/^version\s*=\s*"([^"]+)"/m);
    if (m) return m[1];
  } catch {
    // keep default version
  }
  return "1.0.0";
}

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

function sendFile(res, filePath, downloadName = null) {
  if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
    res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
    res.end("Not Found\n");
    return;
  }
  const ext = path.extname(filePath).toLowerCase();
  const mime = MIME_TYPES[ext] || "application/octet-stream";
  const headers = { "Content-Type": mime };
  if (downloadName) {
    headers["Content-Disposition"] = `attachment; filename="${downloadName}"`;
  }
  res.writeHead(200, headers);
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

function readJsonBody(req, maxBytes = 16384) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on("data", (chunk) => {
      size += chunk.length;
      if (size > maxBytes) {
        reject(new Error("Payload too large"));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      if (chunks.length === 0) return resolve({});
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch {
        reject(new Error("Invalid JSON"));
      }
    });
    req.on("error", reject);
  });
}

const server = http.createServer(async (req, res) => {
  const parsed = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
  const pathname = decodeURIComponent(parsed.pathname);

  if (pathname === "/health" || pathname === "/api/files") {
    return sendJson(res, 200, {
      status: "ok",
      language: "Rust (dog-lang)",
      version: getDogVersion(),
      files: collectFiles(ROOT),
    });
  }

  if (pathname === "/api/workspace") {
    return sendJson(res, 200, {
      version: getDogVersion(),
      docs: listDirSafe(DOCS_DIR, ".md"),
      examples: listDirSafe(EXAMPLES_DIR, [".dg", ".dgss"]),
      templates: collectFiles(TEMPLATES_DIR),
    });
  }

  if (pathname === "/api/file") {
    const kind = parsed.searchParams.get("kind");
    const name = parsed.searchParams.get("name") || "";
    const isTemplate = kind === "template";
    if (
      !name ||
      name.includes("..") ||
      name.includes("\\") ||
      name.startsWith("/") ||
      (!isTemplate && name.includes("/"))
    ) {
      return sendJson(res, 400, { error: "Invalid file name." });
    }
    const baseDir =
      kind === "doc"
        ? DOCS_DIR
        : kind === "example"
        ? EXAMPLES_DIR
        : isTemplate
        ? TEMPLATES_DIR
        : null;
    if (!baseDir) {
      return sendJson(res, 400, { error: "Invalid kind; expected 'doc', 'example', or 'template'." });
    }
    const targetPath = path.resolve(baseDir, name);
    if (!targetPath.startsWith(baseDir + path.sep) || !fs.existsSync(targetPath) || !fs.statSync(targetPath).isFile()) {
      return sendJson(res, 404, { error: "File not found." });
    }
    const content = fs.readFileSync(targetPath, "utf8");
    return sendJson(res, 200, { kind, name, content });
  }

  if (pathname === "/api/readiness" && req.method === "GET") {
    const report = evaluateReadinessGate(ROOT);
    if (parsed.searchParams.get("download") === "1") {
      res.writeHead(200, {
        "Content-Type": "application/json; charset=utf-8",
        "Content-Disposition": 'attachment; filename="dog-v1-readiness-sbom.json"',
      });
      res.end(JSON.stringify(report, null, 2));
      return;
    }
    return sendJson(res, 200, report);
  }

  if (pathname === "/api/stats/event" && req.method === "POST") {
    try {
      const body = await readJsonBody(req);
      siteStatsState = applyStatsEvent(
        siteStatsState,
        body,
        req.headers["user-agent"] || ""
      );
      saveStatsFile(STATS_FILE, siteStatsState);
      return sendJson(res, 200, {
        ok: true,
        site: summarizeSiteStats(siteStatsState),
      });
    } catch (err) {
      return sendJson(res, 400, { ok: false, error: err.message });
    }
  }

  if (pathname === "/api/stats" && req.method === "GET") {
    const roadmap = getRoadmapDataset();
    const publicMetrics = await fetchPublicAdoptionMetrics();
    return sendJson(res, 200, {
      version: getDogVersion(),
      site: summarizeSiteStats(siteStatsState),
      github: publicMetrics,
      codebase: collectCodebaseMetrics(ROOT),
      roadmapSummary: {
        total: roadmap.total,
        countsByStatus: roadmap.countsByStatus,
      },
    });
  }

  if (pathname === "/api/roadmap" && req.method === "GET") {
    const dataset = getRoadmapDataset();
    const result = queryRoadmap(dataset, {
      q: parsed.searchParams.get("q") || "",
      part: parsed.searchParams.get("part") || "ALL",
      subsystem: parsed.searchParams.get("subsystem") || "ALL",
      status: parsed.searchParams.get("status") || "all",
      page: parsed.searchParams.get("page") || "1",
      limit: parsed.searchParams.get("limit") || "50",
    });
    return sendJson(res, 200, result);
  }

  if (pathname === "/update.sh" || pathname === "/setup-apt.sh") {
    const updaterPath = path.join(ROOT, "packaging/termux/setup-apt.sh");
    if (!fs.existsSync(updaterPath) || !fs.statSync(updaterPath).isFile()) {
      res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" });
      res.end("Updater script not found\n");
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/x-shellscript; charset=utf-8",
      "Cache-Control": "no-store",
    });
    fs.createReadStream(updaterPath).pipe(res);
    return;
  }

  if (pathname === "/version.json") {
    const cargoVersion = getDogVersion();
    res.writeHead(200, {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
    });
    res.end(
      JSON.stringify(
        {
          version: cargoVersion,
          source: "workspace",
          updater: "/update.sh",
          updated_at: new Date().toISOString(),
        },
        null,
        2
      )
    );
    return;
  }

  if (Object.prototype.hasOwnProperty.call(ASSET_FILES, pathname)) {
    const dl =
      parsed.searchParams.get("download") === "1"
        ? path.basename(ASSET_FILES[pathname])
        : null;
    return sendFile(res, ASSET_FILES[pathname], dl);
  }

  if (pathname === "/cheatsheet" || pathname === "/cheatsheet.html") {
    return sendFile(res, path.join(WEBSITE_DIR, "cheatsheet.html"));
  }

  if (
    pathname.startsWith("/latest/") ||
    pathname.startsWith("/pool/") ||
    pathname.startsWith("/dists/") ||
    pathname === "/SHA256SUMS"
  ) {
    return sendJson(res, 404, { error: "Repository artifact not hosted on local preview server", path: pathname });
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
