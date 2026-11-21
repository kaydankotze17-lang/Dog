import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const PORT = Number(process.env.PORT || 3000);
const ROOT = process.cwd();

function collectFiles(dir, prefix = "") {
  const results = [];
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

const server = http.createServer((req, res) => {
  const files = collectFiles(ROOT);
  const cargoToml = fs.existsSync(path.join(ROOT, "Cargo.toml"))
    ? fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8")
    : "";

  if (req.url === "/health" || req.url === "/api/files") {
    res.writeHead(200, { "Content-Type": "application/json; charset=utf-8" });
    res.end(JSON.stringify({ status: "ok", language: "Rust (dog-lang)", files }, null, 2));
    return;
  }

  res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
  res.end(
    [
      "DOG Programming Language (Rust Crate Workspace)",
      "===============================================",
      "",
      "This repository is a Rust CLI/interpreter project (not a website).",
      "rustc/cargo are not pre-installed in this AI Studio preview container.",
      "",
      "Cargo.toml:",
      cargoToml.trim(),
      "",
      "Termux / Linux / macOS Build Commands:",
      "  pkg install rust git   # on Termux",
      "  cargo test",
      "  cargo build --release",
      "  cargo run -- run examples/hello.dg",
      "  cargo run -- edit examples/hello.dg",
      "",
      `Workspace files (${files.length}):`,
      ...files.map((f) => `  - ${f}`),
      "",
    ].join("\n")
  );
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`DOG workspace server listening on http://0.0.0.0:${PORT}`);
});
