import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import net from "node:net";
import { execFileSync, spawn, spawnSync } from "node:child_process";

const ROOT = process.cwd();
const SCRIPT_PATH = path.join(ROOT, "packaging/termux/setup-apt.sh");

function createFakeTermuxEnv(t, { installedVer = "0.3.0", remoteVer = "1.0.0", debBytes = null, debShaOverride = null, mode = "apt-ok" } = {}) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dog-updater-test-"));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));

  const prefix = path.join(tmp, "usr");
  const binDir = path.join(tmp, "bin");
  const prefixBin = path.join(prefix, "bin");
  const listsDir = path.join(prefix, "var/lib/apt/lists");
  const archivesDir = path.join(prefix, "var/cache/apt/archives");
  const stateFile = path.join(tmp, "installed_ver");
  const logFile = path.join(tmp, "actions.log");

  fs.mkdirSync(binDir, { recursive: true });
  fs.mkdirSync(prefixBin, { recursive: true });
  fs.mkdirSync(listsDir, { recursive: true });
  fs.mkdirSync(archivesDir, { recursive: true });

  // Seed stale DOG APT caches to verify they get purged before apt-get update
  fs.writeFileSync(path.join(listsDir, "kaydankotze17-lang.github.io_Dog_dists_termux_Release"), "stale\n");
  fs.writeFileSync(path.join(archivesDir, "dog_0.2.0-5_aarch64.deb"), "stale\n");
  fs.writeFileSync(path.join(listsDir, "packages.termux.org_other_Release"), "keep-me\n");

  const validDeb = debBytes || Buffer.from("!<arch>\ndebian-binary   0           0     0     100644  4         `\n2.0\n");
  const debFile = path.join(tmp, "remote.deb");
  fs.writeFileSync(debFile, validDeb);
  const sha = debShaOverride || crypto.createHash("sha256").update(validDeb).digest("hex");

  const versionJson = JSON.stringify({
    version: remoteVer,
    deb_version: `${remoteVer}-9`,
    sha256: { aarch64: sha, arm: sha, x86_64: sha },
  });
  const versionFile = path.join(tmp, "version.json");
  fs.writeFileSync(versionFile, versionJson);
  const cargoFile = path.join(tmp, "Cargo.toml");
  fs.writeFileSync(cargoFile, `[package]\nname = "dog-lang"\nversion = "${remoteVer}"\n`);

  fs.writeFileSync(stateFile, installedVer);

  const dogBin = path.join(prefixBin, "dog");
  fs.writeFileSync(
    dogBin,
    `#!/usr/bin/env bash
if [ "$1" = "--version" ]; then
  v="$(cat "${stateFile}")"
  if [ "$v" = "broken" ]; then exit 1; fi
  echo "dog $v"
  exit 0
fi
exit 0
`,
    { mode: 0o755 }
  );

  // Stub curl reading local test fixtures according to URL
  fs.writeFileSync(
    path.join(binDir, "curl"),
    `#!/usr/bin/env bash
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -H) shift 2 ;;
    --retry|--retry-delay|--connect-timeout|--max-time) shift 2 ;;
    -*) shift ;;
    http://*|https://*) url="$1"; shift ;;
    *) shift ;;
  esac
done
echo "curl:$url" >> "${logFile}"
payload=""
case "$url" in
  *version.json*) payload="${versionFile}" ;;
  *Cargo.toml*) payload="${cargoFile}" ;;
  *dog_latest_*.deb*)
    if [ "${mode}" = "html-then-release" ]; then
      printf '<!DOCTYPE html><html><title>404 Not Found</title></html>' > "$out"
      exit 0
    fi
    payload="${debFile}"
    ;;
  *pool/main/d/dog/*)
    if [ "${mode}" = "html-then-release" ]; then
      printf '<!DOCTYPE html><html><title>404</title></html>' > "$out"
      exit 0
    fi
    payload="${debFile}"
    ;;
  *api.github.com/repos/*/releases/latest*)
    printf '{"assets":[{"browser_download_url":"https://github.com/example/dog_v${remoteVer}_aarch64.deb"}]}' > "$out"
    exit 0
    ;;
  *github.com/example/dog_v*)
    payload="${debFile}"
    ;;
  *SHA256SUMS*)
    exit 1
    ;;
  *)
    exit 1
    ;;
esac
if [ -n "$out" ]; then
  cp "$payload" "$out"
else
  cat "$payload"
fi
`,
    { mode: 0o755 }
  );

  fs.writeFileSync(
    path.join(binDir, "dpkg"),
    `#!/usr/bin/env bash
echo "dpkg:$*" >> "${logFile}"
case "$1" in
  --print-architecture) echo "aarch64"; exit 0 ;;
  --audit) exit 0 ;;
  --configure) exit 0 ;;
  -i)
    if [ "${mode}" = "dpkg-fail-restore" ]; then
      echo "#!/usr/bin/env bash" > "${dogBin}"
      echo "exit 1" >> "${dogBin}"
      chmod +x "${dogBin}"
      exit 1
    fi
    if [ "${mode}" = "postcheck-fail-restore" ]; then
      echo "broken" > "${stateFile}"
      exit 0
    fi
    echo "${remoteVer}" > "${stateFile}"
    exit 0
    ;;
esac
`,
    { mode: 0o755 }
  );

  fs.writeFileSync(
    path.join(binDir, "dpkg-deb"),
    `#!/usr/bin/env bash
if [ "$1" = "-I" ]; then
  head -c 7 "$2" | grep -q "!<arch>" || exit 1
  exit 0
fi
exit 0
`,
    { mode: 0o755 }
  );

  fs.writeFileSync(
    path.join(binDir, "apt-get"),
    `#!/usr/bin/env bash
echo "apt-get:$*" >> "${logFile}"
if [ "${mode}" = "apt-ok" ]; then
  if [ "$1" = "install" ]; then
    echo "${remoteVer}" > "${stateFile}"
  fi
  exit 0
fi
if [ "$1" = "install" ] && [ "${mode}" = "apt-stale" ]; then
  # Simulate stale APT leaving old version in place
  exit 0
fi
exit 0
`,
    { mode: 0o755 }
  );

  const env = {
    ...process.env,
    PREFIX: prefix,
    DOG_TMPDIR: path.join(tmp, "work"),
    PATH: `${prefixBin}:${binDir}:${process.env.PATH}`,
  };

  return { tmp, prefix, listsDir, archivesDir, stateFile, logFile, dogBin, env };
}

test("setup-apt.sh passes bash -n syntax check and prints help", () => {
  execFileSync("bash", ["-n", SCRIPT_PATH]);
  const helpOut = execFileSync("bash", [SCRIPT_PATH, "--help"], { encoding: "utf8" });
  assert.match(helpOut, /DOG Termux Installer & Updater/);
  assert.match(helpOut, /--check/);
  assert.match(helpOut, /--force/);
  assert.match(helpOut, /--from-source/);
});

test("setup-apt.sh --check reports available update cleanly without modifying system", (t) => {
  const { env, listsDir } = createFakeTermuxEnv(t, { installedVer: "0.3.0", remoteVer: "1.0.0" });
  const out = execFileSync("bash", [SCRIPT_PATH, "--check"], { env, encoding: "utf8" });
  assert.match(out, /Installed\s*:\s*0\.3\.0/);
  assert.match(out, /Termux APT\s*:\s*1\.0\.0/);
  assert.match(out, /Update available \(0\.3\.0 -> 1\.0\.0\)/);
  assert.equal(fs.existsSync(path.join(listsDir, "packages.termux.org_other_Release")), true);
});

test("setup-apt.sh purges ONLY stale DOG caches and upgrades cleanly via isolated APT", (t) => {
  const { env, listsDir, archivesDir, stateFile, logFile } = createFakeTermuxEnv(t, {
    installedVer: "0.3.0",
    remoteVer: "1.0.0",
    mode: "apt-ok",
  });
  const out = execFileSync("bash", [SCRIPT_PATH], { env, encoding: "utf8" });
  assert.match(out, /Active DOG version: dog 1\.0\.0/);
  assert.equal(fs.readFileSync(stateFile, "utf8").trim(), "1.0.0");
  // Stale DOG cache removed, unrelated Termux repo preserved
  assert.equal(fs.existsSync(path.join(listsDir, "kaydankotze17-lang.github.io_Dog_dists_termux_Release")), false);
  assert.equal(fs.existsSync(path.join(archivesDir, "dog_0.2.0-5_aarch64.deb")), false);
  assert.equal(fs.existsSync(path.join(listsDir, "packages.termux.org_other_Release")), true);
  assert.match(fs.readFileSync(logFile, "utf8"), /apt-get:update -o Dir::Etc::sourcelist=sources\.list\.d\/dog\.list/);
});

test("setup-apt.sh rejects HTML 404 responses and falls back to GitHub Releases verified .deb when APT lags", (t) => {
  const { env, stateFile, logFile } = createFakeTermuxEnv(t, {
    installedVer: "0.3.0",
    remoteVer: "1.0.0",
    mode: "html-then-release",
  });
  const out = execFileSync("bash", [SCRIPT_PATH], { env, encoding: "utf8" });
  assert.match(out, /Active DOG version: dog 1\.0\.0/);
  assert.equal(fs.readFileSync(stateFile, "utf8").trim(), "1.0.0");
  const logs = fs.readFileSync(logFile, "utf8");
  assert.match(logs, /api\.github\.com\/repos\/kaydankotze17-lang\/Dog\/releases\/latest/);
});

test("setup-apt.sh rejects SHA-256 mismatch and falls back / fails safely", (t) => {
  const { env } = createFakeTermuxEnv(t, {
    installedVer: "0.3.0",
    remoteVer: "1.0.0",
    mode: "apt-stale",
    debShaOverride: "0".repeat(64),
  });
  // Direct latest & pool fail SHA-256 check, then GitHub release fallback succeeds cleanly
  const out = execFileSync("bash", [SCRIPT_PATH], { env, encoding: "utf8" });
  assert.match(out, /Active DOG version: dog 1\.0\.0/);
});

test("setup-apt.sh rolls back previous dog binary when dpkg -i or post-install check fails", (t) => {
  const { env, dogBin } = createFakeTermuxEnv(t, {
    installedVer: "0.3.0",
    remoteVer: "1.0.0",
    mode: "dpkg-fail-restore",
  });
  const res = spawnSync("bash", [SCRIPT_PATH], { env, encoding: "utf8" });
  assert.notEqual(res.status, 0);
  // Existing binary was restored rather than left broken
  const verCheck = execFileSync(dogBin, ["--version"], { encoding: "utf8" }).trim();
  assert.equal(verCheck, "dog 0.3.0");
});

test("server.mjs serves /update.sh as shell script and /version.json as JSON (never <!DOCTYPE html>)", async (t) => {
  const freePort = await new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      srv.close(() => resolve(addr.port));
    });
    srv.on("error", reject);
  });

  const child = spawn("node", [path.join(ROOT, "server.mjs")], {
    cwd: ROOT,
    env: { ...process.env, PORT: String(freePort) },
    stdio: ["ignore", "pipe", "pipe"],
  });
  t.after(() => child.kill("SIGTERM"));

  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Timed out waiting for server.mjs")), 5000);
    child.stdout.on("data", (chunk) => {
      if (String(chunk).includes("listening on")) {
        clearTimeout(timer);
        resolve();
      }
    });
    child.on("error", reject);
  });

  const base = `http://127.0.0.1:${freePort}`;
  const shRes = await fetch(`${base}/update.sh`);
  assert.equal(shRes.status, 200);
  const shText = await shRes.text();
  assert.match(shText, /^#!\/usr\/bin\/env bash/);
  assert.equal(/^\s*(?:<!doctype html|<html)/i.test(shText), false);

  const verRes = await fetch(`${base}/version.json`);
  assert.equal(verRes.status, 200);
  const verJson = await verRes.json();
  assert.equal(verJson.version, "1.0.0");

  const missingDeb = await fetch(`${base}/latest/dog_latest_aarch64.deb`);
  assert.equal(missingDeb.status, 404);
  const missingBody = await missingDeb.text();
  assert.equal(/<!doctype html/i.test(missingBody), false);
});
