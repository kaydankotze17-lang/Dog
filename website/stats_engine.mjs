import fs from "node:fs";
import path from "node:path";

export const ACTIVE_WINDOW_MS = 3 * 60 * 1000;
const MAX_TRACKED_SESSIONS = 5000;

export function parsePlatformFromUserAgent(ua = "") {
  const s = String(ua).toLowerCase();
  if (!s) return "Other";
  if (s.includes("termux") || s.includes("android")) return "Android / Termux";
  if (s.includes("iphone") || s.includes("ipad") || s.includes("ios")) return "iOS";
  if (s.includes("mac os") || s.includes("macintosh")) return "macOS";
  if (s.includes("windows")) return "Windows";
  if (s.includes("linux") || s.includes("x11")) return "Linux";
  return "Other";
}

export function createEmptyStatsState() {
  return {
    startedAt: new Date().toISOString(),
    totalVisits: 0,
    playgroundRuns: 0,
    webStudioBuilds: 0,
    webStudioExports: 0,
    knownSessions: [],
    activeSessions: {},
    platformCounts: {
      "Android / Termux": 0,
      Linux: 0,
      macOS: 0,
      Windows: 0,
      iOS: 0,
      Other: 0,
    },
    lastEventAt: null,
  };
}

export function pruneActiveSessions(activeSessions, nowMs = Date.now()) {
  const next = {};
  for (const [id, info] of Object.entries(activeSessions || {})) {
    if (info && typeof info.lastSeenMs === "number" && nowMs - info.lastSeenMs <= ACTIVE_WINDOW_MS) {
      next[id] = info;
    }
  }
  return next;
}

export function applyStatsEvent(state, payload = {}, userAgent = "", nowMs = Date.now()) {
  const next = {
    ...createEmptyStatsState(),
    ...state,
    knownSessions: Array.isArray(state?.knownSessions) ? [...state.knownSessions] : [],
    platformCounts: { ...createEmptyStatsState().platformCounts, ...(state?.platformCounts || {}) },
    activeSessions: pruneActiveSessions(state?.activeSessions, nowMs),
  };

  const event = String(payload.event || "visit").trim();
  const rawSid = String(payload.sessionId || "").replace(/[^a-zA-Z0-9_-]/g, "").slice(0, 64);
  const sessionId = rawSid || `anon_${nowMs}`;
  const platform = parsePlatformFromUserAgent(userAgent);

  const isNewSession = !next.knownSessions.includes(sessionId);
  if (isNewSession) {
    next.knownSessions.push(sessionId);
    if (next.knownSessions.length > MAX_TRACKED_SESSIONS) {
      next.knownSessions = next.knownSessions.slice(-MAX_TRACKED_SESSIONS);
    }
    next.platformCounts[platform] = (next.platformCounts[platform] || 0) + 1;
  }

  next.activeSessions[sessionId] = {
    lastSeenMs: nowMs,
    platform,
  };

  if (event === "visit") {
    next.totalVisits += 1;
  } else if (event === "playground_run") {
    next.playgroundRuns += 1;
  } else if (event === "web_build") {
    next.webStudioBuilds += 1;
  } else if (event === "web_export") {
    next.webStudioExports += 1;
  }

  next.lastEventAt = new Date(nowMs).toISOString();
  return next;
}

export function summarizeSiteStats(state, nowMs = Date.now()) {
  const active = pruneActiveSessions(state?.activeSessions, nowMs);
  const activeList = Object.values(active);
  const activePlatforms = {};
  for (const entry of activeList) {
    activePlatforms[entry.platform] = (activePlatforms[entry.platform] || 0) + 1;
  }

  return {
    startedAt: state?.startedAt || null,
    lastEventAt: state?.lastEventAt || null,
    activeUsersNow: activeList.length,
    totalVisits: state?.totalVisits || 0,
    uniqueVisitors: Array.isArray(state?.knownSessions) ? state.knownSessions.length : 0,
    playgroundRuns: state?.playgroundRuns || 0,
    webStudioBuilds: state?.webStudioBuilds || 0,
    webStudioExports: state?.webStudioExports || 0,
    platformCounts: state?.platformCounts || createEmptyStatsState().platformCounts,
    activePlatforms,
  };
}

export function loadStatsFile(filePath) {
  try {
    if (!fs.existsSync(filePath)) return createEmptyStatsState();
    const raw = fs.readFileSync(filePath, "utf8");
    const parsed = JSON.parse(raw);
    return {
      ...createEmptyStatsState(),
      ...parsed,
    };
  } catch {
    return createEmptyStatsState();
  }
}

export function saveStatsFile(filePath, state) {
  const tmpPath = `${filePath}.tmp`;
  fs.writeFileSync(tmpPath, JSON.stringify(state, null, 2));
  fs.renameSync(tmpPath, filePath);
}

export function collectCodebaseMetrics(rootDir = process.cwd()) {
  let rustFiles = 0;
  let rustLines = 0;
  let testFiles = 0;
  let docFiles = 0;
  let exampleFiles = 0;

  function countLines(p) {
    try {
      const content = fs.readFileSync(p, "utf8");
      return content.split(/\r?\n/).length;
    } catch {
      return 0;
    }
  }

  function walkRs(dir) {
    if (!fs.existsSync(dir)) return;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walkRs(full);
      else if (entry.name.endsWith(".rs")) {
        rustFiles += 1;
        rustLines += countLines(full);
      }
    }
  }

  walkRs(path.join(rootDir, "src"));
  const testsDir = path.join(rootDir, "tests");
  if (fs.existsSync(testsDir)) {
    testFiles = fs.readdirSync(testsDir).filter((f) => f.endsWith(".rs")).length;
  }
  const docsDir = path.join(rootDir, "docs");
  if (fs.existsSync(docsDir)) {
    docFiles = fs.readdirSync(docsDir).filter((f) => f.endsWith(".md")).length;
  }
  const exDir = path.join(rootDir, "examples");
  if (fs.existsSync(exDir)) {
    exampleFiles = fs.readdirSync(exDir).filter((f) => f.endsWith(".dg") || f.endsWith(".dgss")).length;
  }

  return {
    rustFiles,
    rustLines,
    testFiles,
    docFiles,
    exampleFiles,
    stdModules: 12,
    stdlibBuiltins: 237,
    cargoExternalCrates: 0,
    unsafeBlocks: 0,
  };
}

let githubCache = {
  fetchedAt: 0,
  data: null,
};

async function fetchJsonWithTimeout(url, timeoutMs = 4500) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, {
      signal: controller.signal,
      headers: {
        "User-Agent": "dog-lang-website/1.0.0",
        Accept: "application/vnd.github+json",
      },
    });
    if (!res.ok) {
      return { ok: false, status: res.status, data: null };
    }
    const data = await res.json();
    return { ok: true, status: res.status, data };
  } catch (err) {
    return { ok: false, status: 0, error: err?.message || "Network unavailable", data: null };
  } finally {
    clearTimeout(timer);
  }
}

export async function fetchPublicAdoptionMetrics() {
  const now = Date.now();
  if (githubCache.data && now - githubCache.fetchedAt < 60_000) {
    return githubCache.data;
  }

  const [repoRes, relRes, aptRes] = await Promise.all([
    fetchJsonWithTimeout("https://api.github.com/repos/kaydankotze17-lang/Dog"),
    fetchJsonWithTimeout("https://api.github.com/repos/kaydankotze17-lang/Dog/releases"),
    fetchJsonWithTimeout("https://kaydankotze17-lang.github.io/Dog/version.json"),
  ]);

  let totalReleaseDownloads = null;
  let latestReleaseTag = null;
  let latestDebDownloads = null;
  let releaseCount = 0;

  if (relRes.ok && Array.isArray(relRes.data)) {
    releaseCount = relRes.data.length;
    totalReleaseDownloads = 0;
    if (relRes.data[0]?.tag_name) {
      latestReleaseTag = relRes.data[0].tag_name;
    }
    for (const rel of relRes.data) {
      for (const asset of rel.assets || []) {
        const dl = Number(asset.download_count || 0);
        totalReleaseDownloads += dl;
        if (rel === relRes.data[0] && String(asset.name || "").endsWith(".deb")) {
          latestDebDownloads = (latestDebDownloads || 0) + dl;
        }
      }
    }
  }

  const payload = {
    fetchedAt: new Date(now).toISOString(),
    githubReachable: Boolean(repoRes.ok),
    githubError: repoRes.ok
      ? null
      : repoRes.status
      ? `GitHub API returned HTTP ${repoRes.status}`
      : repoRes.error || "GitHub API unreachable in this environment",
    repo: "kaydankotze17-lang/Dog",
    stars: repoRes.ok ? Number(repoRes.data.stargazers_count || 0) : null,
    forks: repoRes.ok ? Number(repoRes.data.forks_count || 0) : null,
    watchers: repoRes.ok ? Number(repoRes.data.subscribers_count ?? repoRes.data.watchers_count ?? 0) : null,
    openIssues: repoRes.ok ? Number(repoRes.data.open_issues_count || 0) : null,
    pushedAt: repoRes.ok ? repoRes.data.pushed_at || null : null,
    latestReleaseTag,
    releaseCount,
    totalReleaseDownloads,
    latestDebDownloads,
    aptReachable: Boolean(aptRes.ok),
    aptVersion: aptRes.ok ? aptRes.data?.version || aptRes.data?.latest || null : null,
  };

  githubCache = { fetchedAt: now, data: payload };
  return payload;
}
