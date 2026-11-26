import test from "node:test";
import assert from "node:assert/strict";
import {
  applyStatsEvent,
  collectCodebaseMetrics,
  createEmptyStatsState,
  parsePlatformFromUserAgent,
  summarizeSiteStats,
} from "./stats_engine.mjs";

test("tracks unique sessions, active visitors, playground runs, and web builds", () => {
  const now = 1_700_000_000_000;
  let state = createEmptyStatsState();

  state = applyStatsEvent(state, { sessionId: "sess_a", event: "visit" }, "Mozilla/5.0 (Linux; Android 14)", now);
  state = applyStatsEvent(state, { sessionId: "sess_a", event: "playground_run" }, "Mozilla/5.0 (Linux; Android 14)", now + 1000);
  state = applyStatsEvent(state, { sessionId: "sess_b", event: "visit" }, "Mozilla/5.0 (X11; Linux x86_64)", now + 2000);
  state = applyStatsEvent(state, { sessionId: "sess_b", event: "web_build" }, "Mozilla/5.0 (X11; Linux x86_64)", now + 3000);
  state = applyStatsEvent(state, { sessionId: "sess_b", event: "web_export" }, "Mozilla/5.0 (X11; Linux x86_64)", now + 4000);

  const summary = summarizeSiteStats(state, now + 5000);
  assert.equal(summary.totalVisits, 2);
  assert.equal(summary.uniqueVisitors, 2);
  assert.equal(summary.activeUsersNow, 2);
  assert.equal(summary.playgroundRuns, 1);
  assert.equal(summary.webStudioBuilds, 1);
  assert.equal(summary.webStudioExports, 1);
  assert.equal(summary.platformCounts["Android / Termux"], 1);
  assert.equal(summary.platformCounts.Linux, 1);

  // Stale session older than 3 minutes drops from activeUsersNow
  const later = summarizeSiteStats(state, now + 250_000);
  assert.equal(later.activeUsersNow, 0);
  assert.equal(later.uniqueVisitors, 2);
});

test("parsePlatformFromUserAgent classifies common platforms", () => {
  assert.equal(parsePlatformFromUserAgent("Termux/0.118 Android"), "Android / Termux");
  assert.equal(parsePlatformFromUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X)"), "macOS");
  assert.equal(parsePlatformFromUserAgent("Mozilla/5.0 (Windows NT 10.0)"), "Windows");
});

test("collectCodebaseMetrics scans repository counts", () => {
  const metrics = collectCodebaseMetrics(process.cwd());
  assert.ok(metrics.rustFiles > 10);
  assert.ok(metrics.rustLines > 1000);
  assert.ok(metrics.docFiles >= 10);
  assert.equal(metrics.stdModules, 12);
});
