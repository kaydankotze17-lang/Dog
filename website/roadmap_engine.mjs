import fs from "node:fs";
import path from "node:path";

export const PART_DEFINITIONS = [
  {
    id: "A",
    title: "Part A — Wishlist (Core, Web, Studio, Networking, DB, DX)",
    shortLabel: "Part A · Wishlist",
    range: "00001–00150",
    description:
      "Core language evolution, memory & performance, concurrency, security & crypto, networking & APIs, database/SQLite, web components, DOG Studio IDE, debugger/testing, packages, and CLI DX.",
  },
  {
    id: "B",
    title: "Part B — Updater, installation and release engineering",
    shortLabel: "Part B · Updater",
    range: "00151–00200",
    description:
      "Atomic updates, rollback, checksum & signature verification, Termux mirror failover, lockfiles, repair, channels, and updater regression tests.",
  },
  {
    id: "C",
    title: "Part C — Compiler reliability",
    shortLabel: "Part C · Compiler",
    range: "00201–00250",
    description:
      "Lexer/parser error recovery, precise spans, cycle reporting, fuzz & differential tests, ARMv7/AArch64/desktop regression suites, and JSON diagnostics.",
  },
  {
    id: "D",
    title: "Part D — Standard-library quality",
    shortLabel: "Part D · Stdlib QA",
    range: "00251–00300",
    description:
      "Consistent conventions, boundary/UTF-8 safety, path & subprocess timeouts, HTTP/JSON/TOML/CSV diagnostics, crypto test vectors, and socket cleanup.",
  },
  {
    id: "E",
    title: "Part E — Subsystem-specific engineering upgrades",
    shortLabel: "Part E · Subsystems",
    range: "00301–09512",
    description:
      "Targeted reliability, security, compatibility, reproducibility, stress, and CLI/diagnostic upgrades across 95 individual DOG subsystems.",
  },
  {
    id: "F",
    title: "Part F — Release-wide and ecosystem upgrades",
    shortLabel: "Part F · Ecosystem",
    range: "09513–09563",
    description:
      "Formal v1.0.0 compatibility contract, RFC governance, release-candidate gates, project templates, SBOM/license reports, and interpreter-vs-native parity suites.",
  },
  {
    id: "G",
    title: "Part G — Additional validation and maintenance upgrades",
    shortLabel: "Part G · Validation",
    range: "09564–10150",
    description:
      "Edge-case example sets, failure-mode matrices, acceptance tests, compatibility fixture packs, baselines, and checklists per subsystem.",
  },
];

// Grounded against verified DOG v0.3.0 core + newly delivered Part F contracts/templates/readiness & sandbox/web compiler upgrades
const IMPLEMENTED_EXACT_MATCHES = [
  "type aliases",
  "destructuring assignments",
  "optimized string operations",
  "optimized list and map operations",
  "secure comparison utilities",
  "key derivation functions",
  "secure random byte apis with stronger documentation",
  "cryptographic algorithm documentation",
  "http route parameters",
  "http request and response builders",
  "data validation schemas",
  "reusable dog web components",
  "form validation system",
  "static site generation",
  "automatic css prefixing",
  "css tree-shaking",
  "accessibility helpers",
  "breakpoints",
  "step-by-step execution",
  "variable inspection",
  "test coverage reports",
  "benchmark testing",
  "snapshot testing",
  "improved error codes and explanations",
  "machine-readable diagnostic output",
  "dependency graph visualization",
  "package integrity verification",
  "package templates",
  "dog init project templates",
  "version comparison improvements",
  "update diagnostic report",
  "mirror failover",
  "improved module-cycle reporting",
  "type-checker regression tests",
  "cross-platform compiler tests",
  "json diagnostics output",
  "machine-readable compiler errors",
  "diagnostic severity levels",
  "diagnostic source suggestions",
  "standard-library documentation tests",
  "path normalization utilities",
  "json schema validation",
  "cryptographic test vectors",
  "hmac compatibility tests",
  // Part F deliverables created in docs/, templates/, examples/, and website/readiness_engine.mjs
  "add a formal dog language specification with versioned grammar snapshots",
  "publish a v1.0.0 compatibility contract for source code and standard-library apis",
  "create a release-candidate test gate that blocks releases when critical tests fail",
  "create a public known-issues register with severity, workaround, and target version",
  "create a reproducible build guide for every supported operating system and architecture",
  "create a support policy defining which dog versions receive fixes",
  "create a security response policy for reporting and coordinating vulnerability fixes",
  "create a package naming and namespace policy to reduce registry collisions",
  "create a project governance document describing how language changes are proposed and accepted",
  "create a language change proposal template with motivation, syntax, semantics, and compatibility impact",
  "create an rfc review workflow for major language and standard-library changes",
  "create a stable-versus-experimental feature policy",
  "create a formal deprecation window before removing public apis",
  "create a release checklist shared by compiler, runtime, documentation, and packaging maintainers",
  "create a v0.3-to-v1.0 migration guide with automated checks",
  "create a project template for command-line applications",
  "create a project template for http services",
  "create a project template for browser-based applications",
  "create a project template for reusable dog libraries",
  "create a project template for test-driven dog projects",
  "create an end-to-end sample application that exercises the standard library",
  "create a minimal-install distribution profile for constrained devices",
  "create an offline-first documentation bundle for termux users",
  "create an offline package workflow for air-gapped development environments",
  "create an official feature maturity dashboard generated from the capability catalog",
  "create a documentation consistency checker that compares examples with actual exports",
  "create a generated compatibility matrix for os, architecture, and release version",
  "create a regression bisect helper for locating the first failing dog commit",
  "create an sbom export for distributed dog binaries and packages",
  "create a license report for bundled components and dependencies",
  "create a platform-specific troubleshooting guide for installation and updates",
  "create a compatibility test corpus covering both english-style and classic syntax",
  "create a standard-library contract suite that verifies documented signatures",
  "create a v1.0.0 release readiness report with explicit pass/fail criteria",
  // Additional completed features in sandbox.js, dog_web.js, and cheatsheet
  "anonymous functions and lambdas",
  "closures",
  "pattern matching improvements",
  "slice indexing syntax",
  "constant folding",
  "dead-code elimination",
  "unreachable-code detection",
  "aes-256 encryption",
  "ed25519 digital signatures",
  "password hashing utilities",
  "sqlite integration",
  "prepared sql statements",
  "transaction support",
  "query builder",
  "http route grouping",
  "rate limiting middleware",
  "openapi specification generation",
  "reactive state management",
  "dom event delegation",
  "theme inheritance",
  "browser compatibility diagnostics",
  "accessibility audit checks",
  "official language cheat sheet",
  // Simple Style dialect & runtime upgrades
  "simple style",
  "multiline expressions",
  "stack trace",
  "pipe operator",
  "nullish",
  "regular expressions",
  "cookie management",
  "semantic versioning",
];

const IMPLEMENTED_PATTERNS = [
  /machine-readable.*(diagnostic|error|status)|json diagnostics/i,
  /actionable recovery suggestions/i,
  /source-location precision/i,
  /compatibility test corpus covering both english-style and classic syntax/i,
  /documentation consistency checker|official feature maturity dashboard/i,
  /documented failure-mode matrix|security review checklist|maintenance checklist|release-blocking acceptance/i,
  /(constant folding|dead-code elimination|unreachable-code detection|password hashing utilities|aes-256|ed25519|sqlite|prepared sql|query builder|reactive state|event delegation|theme inheritance|browser compatibility diagnostics)/i,
];

const PARTIAL_PATTERNS = [
  /compile-time constants|improved type inference|pattern matching improvements|anonymous functions and lambdas/i,
  /constant folding|dead-code elimination|query builder/i,
  /tls certificate|secure configuration|password hashing utilities/i,
  /route parameters|request body|cookie management|improved dns/i,
  /component-based web|theme inheritance|component-scoped styles|asset bundling|browser compatibility diagnostics/i,
  /multi-file editor tabs|split-editor|symbol navigation|context-aware suggestions|project-wide diagnostics|integrated debugger/i,
  /call-stack inspection|test filtering/i,
  /package version constraints|semantic versioning|package publishing|workspace support/i,
  /dog check static|dog upgrade|interactive project/i,
  /atomic update|automatic update rollback|update integrity|download interruption|update compatibility|update failure diagnostics|previous-version restoration|update lock|corrupted installation|offline update|update cache management|cleanup of obsolete/i,
  /lexer recovery|parser recovery|more precise source spans|type mismatch explanations|duplicate declaration|return-path diagnostics|deterministic compilation|ast validation|armv7 compiler|aarch64 compiler|linux compiler|better compiler exit codes|compiler version metadata|unreachable-code detection|strict compilation mode/i,
  /consistent error conventions|consistent return-type|function argument validation|empty-input handling|unicode handling|utf-8 diagnostics|collection mutation|map key consistency|list bounds|environment-variable|subprocess error|http header|redirect handling|json type|toml error|csv malformed|base64 validation|hexadecimal validation|websocket protocol conformance|cli terminal detection|non-interactive terminal/i,
  /^(add|create).*(edge-case example|compatibility fixture|platform support note|smoke-test workflow|golden tests|malformed-input tests|boundary-condition tests|reproducibility checks|strict-mode checks|structured json diagnostics|performance baseline)/i,
  /create a cross-platform acceptance suite|create an interpreter-versus-native execution parity suite|create a support archive command|create a bug-report command/i,
];

const IMPLEMENTED_SUBSYSTEM_ITEMS = [
  { subsystem: /DOG HTTP server|DOG web DOM bridge|DOG HTML builders|DOG CSS generator|DOG DGSS parser|DOG style compiler|DOG theme system/i, re: /(golden tests|malformed-input tests|boundary-condition tests|reproducibility checks|edge-case example set|platform support note|smoke-test workflow)/i },
  { subsystem: /DOG updater|DOG diagnostics engine|DOG test runner|DOG formatter|DOG cryptography module|DOG bundle builder|DOG bundle verifier/i, re: /(golden tests|actionable recovery suggestions|source-location precision|edge-case example set|acceptance test|platform support note)/i },
  { subsystem: /DOG lexer|DOG parser|DOG type checker|DOG interpreter/i, re: /(golden tests|malformed-input tests|boundary-condition tests|source-location precision|edge-case example set|acceptance test)/i },
];

export function classifyProposalStatus(item) {
  const lower = item.title.toLowerCase();
  const sub = item.subsystem || "";

  if (IMPLEMENTED_EXACT_MATCHES.some((exact) => lower === exact || lower.includes(exact))) {
    return "implemented";
  }
  if (IMPLEMENTED_PATTERNS.some((re) => re.test(item.title))) {
    return "implemented";
  }
  if (IMPLEMENTED_SUBSYSTEM_ITEMS.some((rule) => rule.subsystem.test(sub) && rule.re.test(item.title))) {
    return "implemented";
  }
  if (PARTIAL_PATTERNS.some((re) => re.test(item.title))) {
    return "partial";
  }
  return "planned";
}

function extractPartESubsystem(title) {
  // Handle patterns like:
  // "Add X for DOG lexer." / "Add X to DOG parser." / "Add X of DOG AST."
  const cleaned = title.replace(/\s*\([^)]+\)\s*\.?$/, "").trim();
  const match =
    cleaned.match(/\b(?:for|to|of|in|on)\s+(DOG [^.]+?)\.?$/i) ||
    cleaned.match(/\b(DOG [a-zA-Z0-9 /._-]+?)(?:\.|$)/i);
  if (match) {
    return match[1].replace(/[.,;:]+$/, "").trim();
  }
  return "DOG General";
}

function extractPartGTag(title) {
  const m = title.match(/\((documentation|testing|performance|operations|security|migration)\)\.?$/i);
  return m ? m[1].toLowerCase() : null;
}

export function parseUpgradeIdeasText(rawText) {
  const lines = rawText.split(/\r?\n/);

  const meta = {
    title: "DOG v1.0.0 — 10,150 Upgrade Ideas",
    version: "planning draft",
    scope: "",
    notes: [],
    rawUrl: "/assets/aistudio/DOG_v1.0.0_10150_Upgrade_Ideas_89683764.txt",
  };
  const howToUse = [];
  const items = [];

  let inHowToUse = false;
  let currentPart = "A";
  let currentPartLabel = "PART A — Core language evolution";
  let currentTopic = "Core language evolution";

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      if (inHowToUse && howToUse.length >= 5) inHowToUse = false;
      continue;
    }

    if (items.length === 0 && line.startsWith("#")) {
      const content = line.slice(1).trim();
      if (content.startsWith("DOG v1.0.0")) meta.title = content;
      else if (content.startsWith("Version:")) meta.version = content.replace(/^Version:\s*/i, "");
      else if (content.startsWith("Scope:")) meta.scope = content.replace(/^Scope:\s*/i, "");
      else meta.notes.push(content);
      continue;
    }

    if (line === "HOW TO USE THIS FILE") {
      inHowToUse = true;
      continue;
    }

    if (inHowToUse) {
      const ruleMatch = line.match(/^(\d+)\.\s+(.+)$/);
      if (ruleMatch) {
        howToUse.push({ step: Number(ruleMatch[1]), text: ruleMatch[2].trim() });
        continue;
      }
      inHowToUse = false;
    }

    const partMatch = line.match(/^PART\s+([A-G])\s+[—-]\s+(.+)$/);
    if (partMatch) {
      currentPart = partMatch[1];
      currentPartLabel = line;
      currentTopic = partMatch[2].trim();
      continue;
    }

    const itemMatch = line.match(/^(\d{5})\.\s+(.+)$/);
    if (itemMatch) {
      const sourceNumber = itemMatch[1];
      const title = itemMatch[2].trim();
      const id = items.length + 1;

      let subsystem = currentTopic;
      if (currentPart === "E" || currentPart === "G") {
        subsystem = extractPartESubsystem(title);
      } else if (currentPart === "F") {
        subsystem = "Release-wide & ecosystem";
      } else if (currentPart === "B") {
        subsystem = "Updater & release engineering";
      } else if (currentPart === "C") {
        subsystem = "Compiler reliability";
      } else if (currentPart === "D") {
        subsystem = "Standard-library quality";
      }

      const tag = currentPart === "G" ? extractPartGTag(title) : null;
      const item = {
        id,
        sourceNumber,
        part: currentPart,
        partLabel: currentPartLabel,
        topic: currentTopic,
        subsystem,
        tag,
        title,
      };
      item.status = classifyProposalStatus(item);
      items.push(item);
    }
  }

  // Precompute summaries
  const countsByPart = {};
  for (const def of PART_DEFINITIONS) {
    countsByPart[def.id] = {
      id: def.id,
      title: def.title,
      shortLabel: def.shortLabel,
      range: def.range,
      description: def.description,
      total: 0,
      implemented: 0,
      partial: 0,
      planned: 0,
    };
  }

  const countsByStatus = {
    implemented: 0,
    partial: 0,
    planned: 0,
  };

  const subsystemMap = new Map();
  const partADomainMap = new Map();

  for (const item of items) {
    countsByStatus[item.status] += 1;
    const partBucket = countsByPart[item.part];
    if (partBucket) {
      partBucket.total += 1;
      partBucket[item.status] += 1;
    }

    if (!subsystemMap.has(item.subsystem)) {
      subsystemMap.set(item.subsystem, {
        name: item.subsystem,
        total: 0,
        implemented: 0,
        partial: 0,
        planned: 0,
      });
    }
    const subEntry = subsystemMap.get(item.subsystem);
    subEntry.total += 1;
    subEntry[item.status] += 1;

    if (item.part === "A") {
      if (!partADomainMap.has(item.topic)) {
        partADomainMap.set(item.topic, {
          name: item.topic,
          total: 0,
          implemented: 0,
          partial: 0,
          planned: 0,
        });
      }
      const dom = partADomainMap.get(item.topic);
      dom.total += 1;
      dom[item.status] += 1;
    }
  }

  const subsystems = [...subsystemMap.values()].sort((a, b) => b.total - a.total || a.name.localeCompare(b.name));
  const partADomains = [...partADomainMap.values()];

  return {
    meta,
    howToUse,
    total: items.length,
    countsByPart,
    countsByStatus,
    subsystems,
    partADomains,
    items,
  };
}

export function queryRoadmap(dataset, options = {}) {
  const q = String(options.q || "").trim().toLowerCase();
  const part = String(options.part || "").trim().toUpperCase();
  const subsystem = String(options.subsystem || "").trim();
  const status = String(options.status || "").trim().toLowerCase();
  const pageRaw = Number(options.page || 1);
  const limitRaw = Number(options.limit || 50);
  const limit = Number.isFinite(limitRaw) ? Math.min(Math.max(Math.trunc(limitRaw), 1), 200) : 50;

  let filtered = dataset.items;

  if (part && part !== "ALL") {
    filtered = filtered.filter((it) => it.part === part);
  }
  if (subsystem && subsystem !== "ALL") {
    filtered = filtered.filter((it) => it.subsystem === subsystem);
  }
  if (status && status !== "all") {
    filtered = filtered.filter((it) => it.status === status);
  }
  if (q) {
    const normalizedNum = q.replace(/^#/, "");
    filtered = filtered.filter(
      (it) =>
        it.title.toLowerCase().includes(q) ||
        it.subsystem.toLowerCase().includes(q) ||
        it.topic.toLowerCase().includes(q) ||
        it.sourceNumber.includes(normalizedNum) ||
        String(it.id) === normalizedNum
    );
  }

  const filteredTotal = filtered.length;
  const totalPages = Math.max(1, Math.ceil(filteredTotal / limit));
  const page = Number.isFinite(pageRaw) ? Math.min(Math.max(Math.trunc(pageRaw), 1), totalPages) : 1;
  const start = (page - 1) * limit;
  const items = filtered.slice(start, start + limit);

  return {
    meta: dataset.meta,
    howToUse: dataset.howToUse,
    total: dataset.total,
    filteredTotal,
    page,
    limit,
    totalPages,
    countsByPart: dataset.countsByPart,
    countsByStatus: dataset.countsByStatus,
    subsystems: dataset.subsystems,
    partADomains: dataset.partADomains,
    items,
  };
}

let cachedDataset = null;

export function loadDefaultRoadmap(rootDir = process.cwd()) {
  if (cachedDataset) return cachedDataset;
  const txtPath = path.join(rootDir, "public/assets/aistudio/DOG_v1.0.0_10150_Upgrade_Ideas_89683764.txt");
  const raw = fs.readFileSync(txtPath, "utf8");
  cachedDataset = parseUpgradeIdeasText(raw);
  return cachedDataset;
}
