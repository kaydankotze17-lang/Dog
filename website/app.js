import {
  CLI_COMMANDS,
  INSTALL_METHODS,
  PLAYGROUND_PRESETS,
  STD_MODULES,
  SYNTAX_SHOWCASE,
  WEB_STUDIO_PRESETS,
} from "./data.js";
import { runDogSandbox } from "./sandbox.js";
import { compileDogWebsite } from "./dog_web.js";

const KEYWORDS = new Set([
  "let", "var", "const", "set", "to", "increase", "decrease", "by",
  "if", "then", "otherwise", "else", "elif", "unless", "match", "case",
  "try", "catch", "struct", "enum", "use", "style", "end", "or", "and", "not",
  "is", "equals", "equal", "greater", "less", "than", "at", "least",
  "most", "over", "under", "fn", "define", "function", "returns",
  "return", "while", "until", "repeat", "times", "for", "each", "from",
  "in", "break", "continue", "true", "false",
  "keep", "make", "do", "give", "show", "when", "loop", "stop", "skip",
  "oops", "bump", "add", "take", "check", "null",
]);

const TYPES = new Set(["text", "number", "decimal", "bool", "void", "list", "map"]);

function renderHighlightedCode(container, code) {
  container.textContent = "";
  const lines = code.split("\n");
  lines.forEach((line, lineIdx) => {
    let i = 0;
    while (i < line.length) {
      const ch = line[i];
      if (ch === "#" || (ch === "/" && line[i + 1] === "/")) {
        const sp = document.createElement("span");
        sp.className = "tok-com";
        sp.textContent = line.slice(i);
        container.appendChild(sp);
        break;
      }
      if ((ch === "$" && line[i + 1] === '"') || ch === '"') {
        const start = i;
        i += ch === "$" ? 2 : 1;
        while (i < line.length) {
          if (line[i] === "\\") {
            i += 2;
            continue;
          }
          if (line[i] === '"') {
            i++;
            break;
          }
          i++;
        }
        const sp = document.createElement("span");
        sp.className = "tok-str";
        sp.textContent = line.slice(start, i);
        container.appendChild(sp);
        continue;
      }
      if (/[0-9]/.test(ch)) {
        const start = i;
        while (i < line.length && /[0-9._xbXB]/.test(line[i])) i++;
        const sp = document.createElement("span");
        sp.className = "tok-num";
        sp.textContent = line.slice(start, i);
        container.appendChild(sp);
        continue;
      }
      if (/[a-zA-Z_]/.test(ch)) {
        const start = i;
        while (i < line.length && /[a-zA-Z0-9_]/.test(line[i])) i++;
        const word = line.slice(start, i);
        const sp = document.createElement("span");
        if (KEYWORDS.has(word)) sp.className = "tok-kw";
        else if (TYPES.has(word)) sp.className = "tok-type";
        else if (line[i] === "(") sp.className = "tok-fn";
        sp.textContent = word;
        container.appendChild(sp);
        continue;
      }
      const nextSpecial = line.slice(i).search(/[#"/$0-9a-zA-Z_]/);
      const chunkLen = nextSpecial <= 0 ? 1 : nextSpecial;
      container.appendChild(document.createTextNode(line.slice(i, i + chunkLen)));
      i += chunkLen;
    }
    if (lineIdx < lines.length - 1) {
      container.appendChild(document.createTextNode("\n"));
    }
  });
}

function copyWithFeedback(button, text) {
  const original = button.textContent;
  const finish = () => {
    button.textContent = "Copied";
    setTimeout(() => {
      button.textContent = original;
    }, 1600);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(finish).catch(finish);
  } else {
    finish();
  }
}

document.addEventListener("DOMContentLoaded", () => {
  // Mobile menu
  const menuBtn = document.getElementById("mobile-menu-toggle");
  const mobileDrawer = document.getElementById("mobile-drawer");
  if (menuBtn && mobileDrawer) {
    menuBtn.addEventListener("click", () => {
      const open = mobileDrawer.classList.toggle("is-open");
      menuBtn.setAttribute("aria-expanded", String(open));
    });
    mobileDrawer.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => {
        mobileDrawer.classList.remove("is-open");
        menuBtn.setAttribute("aria-expanded", "false");
      });
    });
  }

  // Telemetry Session Helper
  const SESSION_STORAGE_KEY = "dog_site_session_id";
  let sessionId = "";
  try {
    sessionId = window.sessionStorage.getItem(SESSION_STORAGE_KEY) || "";
    if (!sessionId) {
      sessionId = `dg_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
      window.sessionStorage.setItem(SESSION_STORAGE_KEY, sessionId);
    }
  } catch {
    sessionId = `dg_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
  }

  // Image fallback handlers
  document.querySelectorAll("img[data-fallback-id]").forEach((img) => {
    img.addEventListener("error", () => {
      img.style.display = "none";
      const fb = document.getElementById(img.dataset.fallbackId);
      if (fb) fb.style.display = "flex";
    });
  });

  // 01. Live Usage & Community Adoption Stats
  const statsUpdatedEl = document.getElementById("stats-updated-label");
  const refreshStatsBtn = document.getElementById("refresh-stats-btn");
  const statActiveUsersEl = document.getElementById("stat-active-users");
  const statUniqueVisitorsEl = document.getElementById("stat-unique-visitors");
  const statTotalVisitsEl = document.getElementById("stat-total-visits");
  const statPlaygroundRunsEl = document.getElementById("stat-playground-runs");
  const statWebBuildsEl = document.getElementById("stat-web-builds");
  const statWebExportsEl = document.getElementById("stat-web-exports");
  const statsPlatformRowsEl = document.getElementById("stats-platform-rows");
  const statGhStarsEl = document.getElementById("stat-gh-stars");
  const statGhForksEl = document.getElementById("stat-gh-forks");
  const statGhDownloadsEl = document.getElementById("stat-gh-downloads");
  const statGhDebDownloadsEl = document.getElementById("stat-gh-deb-downloads");
  const statGhReleaseEl = document.getElementById("stat-gh-release");
  const statAptVersionEl = document.getElementById("stat-apt-version");
  const statGhNoteEl = document.getElementById("stat-gh-note");
  const statRustFilesEl = document.getElementById("stat-rust-files");
  const statRustLinesEl = document.getElementById("stat-rust-lines");
  const statExamplesCountEl = document.getElementById("stat-examples-count");
  const statDocsCountEl = document.getElementById("stat-docs-count");
  const statIdeasImplEl = document.getElementById("stat-ideas-implemented");
  const statIdeasPlanEl = document.getElementById("stat-ideas-planned");

  const fmtNum = (val) => (typeof val === "number" && Number.isFinite(val) ? val.toLocaleString("en-US") : "--");

  function renderSiteMetrics(site) {
    if (!site) return;
    if (statActiveUsersEl) statActiveUsersEl.textContent = fmtNum(site.activeUsersNow);
    if (statUniqueVisitorsEl) statUniqueVisitorsEl.textContent = fmtNum(site.uniqueVisitors);
    if (statTotalVisitsEl) statTotalVisitsEl.textContent = fmtNum(site.totalVisits);
    if (statPlaygroundRunsEl) statPlaygroundRunsEl.textContent = fmtNum(site.playgroundRuns);
    if (statWebBuildsEl) statWebBuildsEl.textContent = fmtNum(site.webStudioBuilds);
    if (statWebExportsEl) statWebExportsEl.textContent = fmtNum(site.webStudioExports);

    if (statsPlatformRowsEl && site.platformCounts) {
      statsPlatformRowsEl.textContent = "";
      const entries = Object.entries(site.platformCounts).filter(([, c]) => c > 0);
      if (entries.length === 0) {
        const emptyRow = document.createElement("div");
        emptyRow.className = "stats-breakdown-row";
        emptyRow.innerHTML = "<span>No sessions recorded yet</span><strong>0</strong>";
        statsPlatformRowsEl.appendChild(emptyRow);
      } else {
        entries
          .sort((a, b) => b[1] - a[1])
          .forEach(([platform, count]) => {
            const row = document.createElement("div");
            row.className = "stats-breakdown-row";
            const lbl = document.createElement("span");
            lbl.textContent = platform;
            const val = document.createElement("strong");
            val.textContent = `${fmtNum(count)} session${count === 1 ? "" : "s"}`;
            row.appendChild(lbl);
            row.appendChild(val);
            statsPlatformRowsEl.appendChild(row);
          });
      }
    }
  }

  function renderFullStats(data) {
    if (!data) return;
    renderSiteMetrics(data.site);

    const gh = data.github || {};
    if (statGhStarsEl) statGhStarsEl.textContent = gh.githubReachable ? fmtNum(gh.stars) : "Offline";
    if (statGhForksEl) statGhForksEl.textContent = gh.githubReachable ? fmtNum(gh.forks) : "--";
    if (statGhDownloadsEl) {
      statGhDownloadsEl.textContent =
        typeof gh.totalReleaseDownloads === "number" ? fmtNum(gh.totalReleaseDownloads) : "--";
    }
    if (statGhDebDownloadsEl) {
      statGhDebDownloadsEl.textContent =
        typeof gh.latestDebDownloads === "number"
          ? fmtNum(gh.latestDebDownloads)
          : gh.githubReachable
          ? "0 (pending)"
          : "--";
    }
    if (statGhReleaseEl) statGhReleaseEl.textContent = gh.latestReleaseTag || "--";
    if (statAptVersionEl) {
      statAptVersionEl.textContent =
        gh.aptReachable && gh.aptVersion
          ? String(gh.aptVersion)
          : gh.aptReachable
          ? "Active"
          : gh.githubReachable
          ? "Pending"
          : "--";
    }
    if (statGhNoteEl) {
      if (gh.githubReachable) {
        const synced = gh.fetchedAt ? new Date(gh.fetchedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "now";
        const aptNote = gh.aptReachable ? "Termux APT live." : "Termux APT awaiting gh-pages release.";
        statGhNoteEl.textContent = `Synced live from GitHub API (${gh.repo}) at ${synced}. ${gh.releaseCount || 0} release(s), ${fmtNum(gh.openIssues)} open issue(s). ${aptNote}`;
      } else {
        statGhNoteEl.textContent = `${gh.githubError || "GitHub API unreachable in this sandbox"} — local server usage counters remain active.`;
      }
    }

    const cb = data.codebase || {};
    if (statRustFilesEl) statRustFilesEl.textContent = fmtNum(cb.rustFiles);
    if (statRustLinesEl) statRustLinesEl.textContent = fmtNum(cb.rustLines);
    if (statExamplesCountEl) statExamplesCountEl.textContent = fmtNum(cb.exampleFiles);
    if (statDocsCountEl) statDocsCountEl.textContent = fmtNum(cb.docFiles);

    const rm = data.roadmapSummary || {};
    const st = rm.countsByStatus || {};
    if (statIdeasImplEl) statIdeasImplEl.textContent = fmtNum(st.implemented);
    if (statIdeasPlanEl) {
      const remaining = typeof st.planned === "number" && typeof st.partial === "number" ? st.planned + st.partial : null;
      statIdeasPlanEl.textContent = fmtNum(remaining);
    }
    if (statsUpdatedEl) {
      statsUpdatedEl.textContent = `Updated ${new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}`;
    }
  }

  async function fetchStatsSnapshot() {
    try {
      const res = await fetch("/api/stats");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      renderFullStats(data);
    } catch {
      if (statsUpdatedEl) statsUpdatedEl.textContent = "Stats API unavailable";
    }
  }

  async function recordSiteEvent(eventName) {
    try {
      const res = await fetch("/api/stats/event", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sessionId, event: eventName }),
      });
      if (!res.ok) return;
      const data = await res.json();
      if (data?.site) renderSiteMetrics(data.site);
    } catch {
      // Non-fatal in static preview mode
    }
  }

  refreshStatsBtn?.addEventListener("click", () => {
    if (statsUpdatedEl) statsUpdatedEl.textContent = "Refreshing...";
    fetchStatsSnapshot();
  });

  // 1. Syntax Showcase
  let activeTopic = "english";
  let activeDialect = "english";
  const syntaxTabButtons = document.querySelectorAll("[data-syntax-topic]");
  const dialectButtons = document.querySelectorAll("[data-syntax-dialect]");
  const syntaxCodeEl = document.getElementById("syntax-code-view");
  const syntaxFileEl = document.getElementById("syntax-file-label");
  const syntaxNotesEl = document.getElementById("syntax-notes-list");
  const copySyntaxBtn = document.getElementById("copy-syntax-btn");
  const openInPlaygroundBtn = document.getElementById("open-in-playground-btn");

  function getActiveSyntaxCode() {
    const item = SYNTAX_SHOWCASE[activeTopic];
    if (activeDialect === "simple") return item.simpleCode || item.englishCode;
    return activeDialect === "english" ? item.englishCode : item.classicCode;
  }

  function renderSyntaxShowcase() {
    const item = SYNTAX_SHOWCASE[activeTopic];
    const code = getActiveSyntaxCode();
    syntaxTabButtons.forEach((btn) => {
      const isMatch = btn.dataset.syntaxTopic === activeTopic;
      btn.classList.toggle("is-active", isMatch);
      btn.setAttribute("aria-selected", String(isMatch));
    });
    dialectButtons.forEach((btn) => {
      const isMatch = btn.dataset.syntaxDialect === activeDialect;
      btn.classList.toggle("is-active", isMatch);
      btn.setAttribute("aria-selected", String(isMatch));
    });
    const dialectLabel =
      activeDialect === "simple"
        ? "Simple style"
        : activeDialect === "english"
        ? "English style"
        : "Classic style";
    syntaxFileEl.textContent = `${item.file} (${dialectLabel})`;
    renderHighlightedCode(syntaxCodeEl, code);
    syntaxNotesEl.textContent = "";
    item.notes.forEach((note) => {
      const li = document.createElement("li");
      const parts = note.split(/(`[^`]+`)/g);
      parts.forEach((part) => {
        if (part.startsWith("`") && part.endsWith("`")) {
          const codeNode = document.createElement("code");
          codeNode.textContent = part.slice(1, -1);
          li.appendChild(codeNode);
        } else {
          li.appendChild(document.createTextNode(part));
        }
      });
      syntaxNotesEl.appendChild(li);
    });
  }

  syntaxTabButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeTopic = btn.dataset.syntaxTopic;
      renderSyntaxShowcase();
    });
  });

  dialectButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeDialect = btn.dataset.syntaxDialect;
      renderSyntaxShowcase();
    });
  });

  copySyntaxBtn?.addEventListener("click", () => {
    copyWithFeedback(copySyntaxBtn, getActiveSyntaxCode());
  });

  // 2. Live Browser Playground
  const presetSelect = document.getElementById("playground-preset");
  const editorTextarea = document.getElementById("playground-editor");
  const runBtn = document.getElementById("playground-run-btn");
  const resetBtn = document.getElementById("playground-reset-btn");
  const outputStatus = document.getElementById("playground-status");
  const outputBody = document.getElementById("playground-output");
  const outputTabBtns = document.querySelectorAll("[data-output-tab]");
  let activeOutputTab = "console";
  let lastSandboxResult = null;

  PLAYGROUND_PRESETS.forEach((preset) => {
    const opt = document.createElement("option");
    opt.value = preset.id;
    opt.textContent = preset.label;
    presetSelect.appendChild(opt);
  });

  function renderPlaygroundOutput() {
    outputTabBtns.forEach((btn) => {
      const match = btn.dataset.outputTab === activeOutputTab;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });
    if (!lastSandboxResult) return;
    outputBody.textContent = "";
    if (lastSandboxResult.ok) {
      const warnCount = Array.isArray(lastSandboxResult.warnings) ? lastSandboxResult.warnings.length : 0;
      outputStatus.className = "output-status is-ok";
      outputStatus.textContent = `Passed · ${lastSandboxResult.steps} steps · ${lastSandboxResult.elapsedMs} ms${warnCount ? ` · ${warnCount} warning${warnCount === 1 ? "" : "s"}` : ""}`;
      if (activeOutputTab === "console") {
        outputBody.textContent =
          lastSandboxResult.output.length > 0
            ? lastSandboxResult.output.join("\n")
            : "(Program completed with no console output.)";
      } else if (activeOutputTab === "json") {
        outputBody.textContent = lastSandboxResult.diagnosticsJson || "{}";
      } else {
        outputBody.textContent =
          lastSandboxResult.astSummary.length > 0
            ? lastSandboxResult.astSummary.join("\n")
            : "(No top-level declarations recorded.)";
      }
    } else {
      outputStatus.className = "output-status is-err";
      outputStatus.textContent = `Diagnostic ${lastSandboxResult.errorCode || "E004"}`;
      if (activeOutputTab === "console") {
        const box = document.createElement("div");
        box.className = "output-error-box";
        box.textContent = lastSandboxResult.error;
        outputBody.appendChild(box);
        if (lastSandboxResult.output.length > 0) {
          const prior = document.createElement("pre");
          prior.style.margin = "12px 16px";
          prior.style.fontFamily = "var(--font-mono)";
          prior.textContent = `Prior output:\n${lastSandboxResult.output.join("\n")}`;
          outputBody.appendChild(prior);
        }
      } else if (activeOutputTab === "json") {
        outputBody.textContent = lastSandboxResult.diagnosticsJson || "{}";
      } else {
        outputBody.textContent = lastSandboxResult.astSummary.join("\n") || "Stopped before type trace completion.";
      }
    }
  }

  function executePlayground(trackUsage = false) {
    lastSandboxResult = runDogSandbox(editorTextarea.value);
    renderPlaygroundOutput();
    if (trackUsage) {
      recordSiteEvent("playground_run");
    }
  }

  function loadPreset(id, trackUsage = false) {
    const preset = PLAYGROUND_PRESETS.find((p) => p.id === id) || PLAYGROUND_PRESETS[0];
    presetSelect.value = preset.id;
    editorTextarea.value = preset.code;
    executePlayground(trackUsage);
  }

  presetSelect.addEventListener("change", () => loadPreset(presetSelect.value, true));
  runBtn.addEventListener("click", () => executePlayground(true));
  resetBtn.addEventListener("click", () => loadPreset(presetSelect.value, false));
  outputTabBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeOutputTab = btn.dataset.outputTab;
      renderPlaygroundOutput();
    });
  });

  openInPlaygroundBtn?.addEventListener("click", () => {
    editorTextarea.value = getActiveSyntaxCode();
    executePlayground(true);
    document.getElementById("playground")?.scrollIntoView({ behavior: "smooth" });
  });

  // 2.5 Web & CSS Studio
  const webPresetSelect = document.getElementById("web-studio-preset");
  const webTreeshakeToggle = document.getElementById("web-studio-treeshake-toggle");
  const webMinifyToggle = document.getElementById("web-studio-minify-toggle");
  const webDgEditor = document.getElementById("web-studio-dg-editor");
  const webDgssEditor = document.getElementById("web-studio-dgss-editor");
  const webBuildBtn = document.getElementById("web-studio-build-btn");
  const webResetBtn = document.getElementById("web-studio-reset-btn");
  const webDownloadBtn = document.getElementById("web-studio-download-btn");
  const webStatusEl = document.getElementById("web-studio-status");
  const webPreviewWrap = document.getElementById("web-studio-preview-wrap");
  const webIframeEl = document.getElementById("web-studio-iframe");
  const webCodeOutEl = document.getElementById("web-studio-code-output");
  const webLogLineEl = document.getElementById("web-studio-log-line");
  const webFileTabs = document.querySelectorAll("[data-web-file]");
  const webViewTabs = document.querySelectorAll("[data-web-view]");

  let activeWebFile = "dg";
  let activeWebView = "preview";
  let lastWebBuild = null;

  if (webPresetSelect) {
    WEB_STUDIO_PRESETS.forEach((p) => {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.label;
      webPresetSelect.appendChild(opt);
    });
  }

  function renderWebFileEditor() {
    webFileTabs.forEach((btn) => {
      const match = btn.dataset.webFile === activeWebFile;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });
    if (webDgEditor && webDgssEditor) {
      webDgEditor.style.display = activeWebFile === "dg" ? "block" : "none";
      webDgssEditor.style.display = activeWebFile === "dgss" ? "block" : "none";
    }
  }

  function renderWebStudioOutput() {
    webViewTabs.forEach((btn) => {
      const match = btn.dataset.webView === activeWebView;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });
    if (!lastWebBuild || !webStatusEl || !webPreviewWrap || !webCodeOutEl) return;

    if (!lastWebBuild.ok) {
      webStatusEl.className = "output-status is-err";
      webStatusEl.textContent = `Diagnostic ${lastWebBuild.errorCode || "E004"}`;
      webPreviewWrap.style.display = "none";
      webCodeOutEl.style.display = "block";
      webCodeOutEl.textContent = "";
      if (activeWebView === "json") {
        webCodeOutEl.textContent = lastWebBuild.diagnosticsJson || "{}";
        return;
      }
      const box = document.createElement("div");
      box.className = "output-error-box";
      box.textContent = lastWebBuild.error;
      webCodeOutEl.appendChild(box);
      return;
    }

    webStatusEl.className = "output-status is-ok";
    const modes = [];
    if (webTreeshakeToggle?.checked) modes.push("tree-shaken");
    if (webMinifyToggle?.checked) modes.push("minified");
    const prunedCount = Array.isArray(lastWebBuild.prunedSelectors) ? lastWebBuild.prunedSelectors.length : 0;
    webStatusEl.textContent = modes.length
      ? `Built (${modes.join(" + ")}${prunedCount ? ` · -${prunedCount} unused` : ""})`
      : "Built (.dg + .dgss)";

    if (activeWebView === "preview") {
      webPreviewWrap.style.display = "flex";
      webCodeOutEl.style.display = "none";
    } else {
      webPreviewWrap.style.display = "none";
      webCodeOutEl.style.display = "block";
      if (activeWebView === "css") {
        webCodeOutEl.textContent = lastWebBuild.css || "/* Empty stylesheet */";
      } else if (activeWebView === "js") {
        webCodeOutEl.textContent = lastWebBuild.js || "// Empty script";
      } else if (activeWebView === "json") {
        webCodeOutEl.textContent = lastWebBuild.diagnosticsJson || "{}";
      } else {
        webCodeOutEl.textContent = lastWebBuild.indexHtml;
      }
    }
  }

  function buildWebStudio(trackUsage = false) {
    if (!webDgEditor || !webDgssEditor || !webPresetSelect) return;
    const preset = WEB_STUDIO_PRESETS.find((p) => p.id === webPresetSelect.value) || WEB_STUDIO_PRESETS[0];
    const result = compileDogWebsite({
      code: webDgEditor.value,
      css: webDgssEditor.value,
      title: preset.title || "DOG Web App",
      minify: Boolean(webMinifyToggle?.checked),
      treeShake: Boolean(webTreeshakeToggle?.checked),
    });
    lastWebBuild = result;
    if (result.ok && webIframeEl) {
      webIframeEl.srcdoc = result.indexHtml;
      if (webLogLineEl) {
        webLogLineEl.textContent = "Preview Console: build succeeded — interact with the compiled DOG website above.";
      }
    }
    renderWebStudioOutput();
    if (trackUsage) {
      recordSiteEvent("web_build");
    }
  }

  function loadWebPreset(id, trackUsage = false) {
    const preset = WEB_STUDIO_PRESETS.find((p) => p.id === id) || WEB_STUDIO_PRESETS[0];
    if (webPresetSelect) webPresetSelect.value = preset.id;
    if (webDgEditor) webDgEditor.value = preset.code;
    if (webDgssEditor) webDgssEditor.value = preset.css;
    buildWebStudio(trackUsage);
  }

  webFileTabs.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeWebFile = btn.dataset.webFile;
      renderWebFileEditor();
    });
  });

  webViewTabs.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeWebView = btn.dataset.webView;
      renderWebStudioOutput();
    });
  });

  webPresetSelect?.addEventListener("change", () => loadWebPreset(webPresetSelect.value, true));
  webTreeshakeToggle?.addEventListener("change", () => buildWebStudio(false));
  webMinifyToggle?.addEventListener("change", () => buildWebStudio(false));
  webBuildBtn?.addEventListener("click", () => buildWebStudio(true));
  webResetBtn?.addEventListener("click", () => loadWebPreset(webPresetSelect?.value || WEB_STUDIO_PRESETS[0].id, false));

  webDownloadBtn?.addEventListener("click", () => {
    buildWebStudio(false);
    if (!lastWebBuild?.ok) return;
    recordSiteEvent("web_export");
    const blob = new Blob([lastWebBuild.indexHtml], { type: "text/html;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "index.html";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  });

  window.addEventListener("message", (event) => {
    if (event.data?.source === "dog-web-preview" && event.data?.type === "log" && webLogLineEl) {
      webLogLineEl.textContent = `Preview Console > ${event.data.line}`;
    }
  });

  // 3. Standard Library Explorer
  let activeModuleId = STD_MODULES[0].id;
  let stdQuery = "";
  const modTabsContainer = document.getElementById("stdlib-module-tabs");
  const stdSearchInput = document.getElementById("stdlib-search");
  const stdSummaryEl = document.getElementById("stdlib-module-summary");
  const stdFnRowsEl = document.getElementById("stdlib-fn-rows");
  const stdExampleCodeEl = document.getElementById("stdlib-example-code");
  const copyStdExampleBtn = document.getElementById("copy-stdlib-example-btn");

  STD_MODULES.forEach((mod) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "tab-btn";
    btn.dataset.modId = mod.id;
    btn.textContent = mod.title;
    btn.addEventListener("click", () => {
      activeModuleId = mod.id;
      stdQuery = "";
      stdSearchInput.value = "";
      renderStdlib();
    });
    modTabsContainer.appendChild(btn);
  });

  function renderStdlib() {
    const q = stdQuery.trim().toLowerCase();
    modTabsContainer.querySelectorAll(".tab-btn").forEach((btn) => {
      const match = !q && btn.dataset.modId === activeModuleId;
      btn.classList.toggle("is-active", match);
    });

    const activeMod = STD_MODULES.find((m) => m.id === activeModuleId) || STD_MODULES[0];
    let rows = [];
    if (q) {
      STD_MODULES.forEach((m) => {
        m.functions.forEach((fn) => {
          if (
            fn.name.toLowerCase().includes(q) ||
            fn.desc.toLowerCase().includes(q) ||
            m.title.toLowerCase().includes(q)
          ) {
            rows.push({ ...fn, mod: m.title });
          }
        });
      });
      stdSummaryEl.textContent = `${rows.length} matching exports across 12 std.* modules for "${stdQuery.trim()}"`;
    } else {
      rows = activeMod.functions.map((fn) => ({ ...fn, mod: activeMod.title }));
      stdSummaryEl.textContent = `${activeMod.title} — ${activeMod.summary}`;
    }

    renderHighlightedCode(stdExampleCodeEl, activeMod.example);
    stdFnRowsEl.textContent = "";
    if (rows.length === 0) {
      const emptyRow = document.createElement("div");
      emptyRow.className = "fn-row";
      const p = document.createElement("p");
      p.className = "fn-desc";
      p.textContent = `No standard library functions matched "${stdQuery.trim()}". Clear the filter to browse all modules.`;
      emptyRow.appendChild(p);
      stdFnRowsEl.appendChild(emptyRow);
      return;
    }

    rows.forEach((fn) => {
      const row = document.createElement("div");
      row.className = "fn-row";
      const left = document.createElement("div");
      const nameEl = document.createElement("div");
      nameEl.className = "fn-name";
      nameEl.textContent = q ? `${fn.mod}.${fn.name}` : fn.name;
      const sigEl = document.createElement("span");
      sigEl.className = "fn-sig";
      sigEl.textContent = fn.sig;
      left.appendChild(nameEl);
      left.appendChild(sigEl);

      const descEl = document.createElement("p");
      descEl.className = "fn-desc";
      descEl.textContent = fn.desc;
      row.appendChild(left);
      row.appendChild(descEl);
      stdFnRowsEl.appendChild(row);
    });
  }

  stdSearchInput.addEventListener("input", () => {
    stdQuery = stdSearchInput.value;
    renderStdlib();
  });

  copyStdExampleBtn?.addEventListener("click", () => {
    const mod = STD_MODULES.find((m) => m.id === activeModuleId) || STD_MODULES[0];
    copyWithFeedback(copyStdExampleBtn, mod.example);
  });

  // 4. Install & Toolchain
  let activeInstallId = INSTALL_METHODS[0].id;
  const installTabsEl = document.getElementById("install-tabs");
  const installSubtitleEl = document.getElementById("install-subtitle");
  const installCmdPrimaryEl = document.getElementById("install-cmd-primary");
  const installAltLabelEl = document.getElementById("install-alt-label");
  const installCmdAltEl = document.getElementById("install-cmd-alt");
  const copyInstallBtn = document.getElementById("copy-install-btn");
  const cliListEl = document.getElementById("cli-command-list");

  INSTALL_METHODS.forEach((m) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "tab-btn";
    btn.dataset.installId = m.id;
    btn.textContent = m.label;
    btn.addEventListener("click", () => {
      activeInstallId = m.id;
      renderInstall();
    });
    installTabsEl.appendChild(btn);
  });

  function renderInstall() {
    const method = INSTALL_METHODS.find((m) => m.id === activeInstallId) || INSTALL_METHODS[0];
    installTabsEl.querySelectorAll(".tab-btn").forEach((btn) => {
      btn.classList.toggle("is-active", btn.dataset.installId === activeInstallId);
    });
    installSubtitleEl.textContent = method.subtitle;
    installCmdPrimaryEl.textContent = method.commands;
    installAltLabelEl.textContent = method.altLabel;
    installCmdAltEl.textContent = method.altCommands;
  }

  copyInstallBtn.addEventListener("click", () => {
    const method = INSTALL_METHODS.find((m) => m.id === activeInstallId) || INSTALL_METHODS[0];
    copyWithFeedback(copyInstallBtn, method.commands);
  });

  CLI_COMMANDS.forEach((entry) => {
    const row = document.createElement("div");
    row.className = "cli-item";
    const cmd = document.createElement("div");
    cmd.className = "cli-cmd";
    cmd.textContent = entry.cmd;
    const desc = document.createElement("p");
    desc.className = "cli-desc";
    desc.textContent = entry.desc;
    row.appendChild(cmd);
    row.appendChild(desc);
    cliListEl.appendChild(row);
  });

  // 5. Repository Docs & Examples Viewer
  const docsListEl = document.getElementById("docs-sidebar-list");
  const docTitleEl = document.getElementById("doc-viewer-title");
  const docContentEl = document.getElementById("doc-viewer-content");
  const copyDocBtn = document.getElementById("copy-doc-btn");

  async function loadDocFile(kind, name) {
    const dirLabel = kind === "doc" ? "docs" : kind === "template" ? "templates" : "examples";
    docTitleEl.textContent = `${dirLabel}/${name}`;
    docContentEl.textContent = "Loading workspace file...";
    docsListEl.querySelectorAll(".doc-item-btn").forEach((b) => {
      b.classList.toggle("is-active", b.dataset.kind === kind && b.dataset.name === name);
    });
    try {
      const res = await fetch(`/api/file?kind=${encodeURIComponent(kind)}&name=${encodeURIComponent(name)}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      docContentEl.textContent = data.content || "";
    } catch (err) {
      docContentEl.textContent = `Could not load file (${err.message}).`;
    }
  }

  async function initWorkspaceBrowser() {
    try {
      const res = await fetch("/api/workspace");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      docsListEl.textContent = "";

      const docsLabel = document.createElement("div");
      docsLabel.className = "doc-group-label";
      docsLabel.textContent = "Documentation";
      docsListEl.appendChild(docsLabel);

      (data.docs || []).forEach((name) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "doc-item-btn";
        btn.dataset.kind = "doc";
        btn.dataset.name = name;
        btn.textContent = name;
        btn.addEventListener("click", () => loadDocFile("doc", name));
        docsListEl.appendChild(btn);
      });

      const exLabel = document.createElement("div");
      exLabel.className = "doc-group-label";
      exLabel.textContent = "Examples (.dg)";
      docsListEl.appendChild(exLabel);

      (data.examples || []).forEach((name) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "doc-item-btn";
        btn.dataset.kind = "example";
        btn.dataset.name = name;
        btn.textContent = name;
        btn.addEventListener("click", () => loadDocFile("example", name));
        docsListEl.appendChild(btn);
      });

      if (Array.isArray(data.templates) && data.templates.length > 0) {
        const tplLabel = document.createElement("div");
        tplLabel.className = "doc-group-label";
        tplLabel.textContent = "Templates (5 official)";
        docsListEl.appendChild(tplLabel);

        data.templates.forEach((name) => {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "doc-item-btn";
          btn.dataset.kind = "template";
          btn.dataset.name = name;
          btn.textContent = name;
          btn.addEventListener("click", () => loadDocFile("template", name));
          docsListEl.appendChild(btn);
        });
      }

      if (data.docs?.includes("STANDARD_LIBRARY.md")) {
        loadDocFile("doc", "STANDARD_LIBRARY.md");
      } else if (data.docs?.length) {
        loadDocFile("doc", data.docs[0]);
      }
    } catch {
      docContentEl.textContent = "Workspace API unavailable in static mode. Browse examples/ and docs/ directly in the repository.";
    }
  }

  copyDocBtn.addEventListener("click", () => {
    copyWithFeedback(copyDocBtn, docContentEl.textContent || "");
  });

  // 08. DOG v1.0.0 — 10,150 Upgrade Ideas Explorer
  const roadmapScopeLeadEl = document.getElementById("roadmap-scope-lead");
  const roadmapHowToEl = document.getElementById("roadmap-how-to-use");
  const roadmapNotesEl = document.getElementById("roadmap-file-notes");
  const auditTotalEl = document.getElementById("audit-total-count");
  const auditImplEl = document.getElementById("audit-implemented-count");
  const auditPartEl = document.getElementById("audit-partial-count");
  const auditPlanEl = document.getElementById("audit-planned-count");
  const auditBarImplEl = document.getElementById("audit-bar-implemented");
  const auditBarPartEl = document.getElementById("audit-bar-partial");
  const auditBarPlanEl = document.getElementById("audit-bar-planned");
  const roadmapPartADomainsEl = document.getElementById("roadmap-part-a-domains");
  const roadmapPartTabsEl = document.getElementById("roadmap-part-tabs");
  const roadmapPartSummaryEl = document.getElementById("roadmap-part-summary");
  const roadmapSearchEl = document.getElementById("roadmap-search");
  const roadmapSubsystemEl = document.getElementById("roadmap-subsystem");
  const roadmapStatusTabs = document.querySelectorAll("[data-roadmap-status]");
  const roadmapClearBtn = document.getElementById("roadmap-clear-btn");
  const roadmapResultSummaryEl = document.getElementById("roadmap-result-summary");
  const roadmapPageLabelEl = document.getElementById("roadmap-page-label");
  const roadmapPrevBtn = document.getElementById("roadmap-prev-btn");
  const roadmapNextBtn = document.getElementById("roadmap-next-btn");
  const roadmapRowsEl = document.getElementById("roadmap-rows");
  const roadmapFooterRangeEl = document.getElementById("roadmap-footer-range");
  const roadmapJumpInput = document.getElementById("roadmap-jump-page");
  const roadmapJumpBtn = document.getElementById("roadmap-jump-btn");

  const roadmapState = {
    part: "ALL",
    subsystem: "ALL",
    status: "all",
    q: "",
    page: 1,
    limit: 50,
    initializedMeta: false,
    partDescriptions: {},
    totalPages: 1,
  };

  let roadmapDebounceTimer = null;

  function formatStatusLabel(status) {
    if (status === "implemented") return "Implemented";
    if (status === "partial") return "Partial";
    return "Planned v1.0";
  }

  function renderRoadmapMeta(data) {
    if (!data || roadmapState.initializedMeta) return;
    roadmapState.initializedMeta = true;

    if (roadmapScopeLeadEl && data.meta?.scope) {
      roadmapScopeLeadEl.textContent = `Version: ${data.meta.version || "planning draft"} · Scope: ${data.meta.scope}`;
    }

    if (roadmapHowToEl && Array.isArray(data.howToUse) && data.howToUse.length > 0) {
      roadmapHowToEl.textContent = "";
      data.howToUse.forEach((rule) => {
        const li = document.createElement("li");
        li.textContent = rule.text;
        roadmapHowToEl.appendChild(li);
      });
    }

    if (roadmapNotesEl && Array.isArray(data.meta?.notes)) {
      roadmapNotesEl.textContent = data.meta.notes.join(" ");
    }

    const total = data.total || 10150;
    const cs = data.countsByStatus || { implemented: 0, partial: 0, planned: total };
    if (auditTotalEl) auditTotalEl.textContent = fmtNum(total);
    if (auditImplEl) auditImplEl.textContent = fmtNum(cs.implemented);
    if (auditPartEl) auditPartEl.textContent = fmtNum(cs.partial);
    if (auditPlanEl) auditPlanEl.textContent = fmtNum(cs.planned);

    const pImpl = ((cs.implemented / total) * 100).toFixed(1);
    const pPart = ((cs.partial / total) * 100).toFixed(1);
    const pPlan = Math.max(0, 100 - Number(pImpl) - Number(pPart)).toFixed(1);
    if (auditBarImplEl) auditBarImplEl.style.width = `${pImpl}%`;
    if (auditBarPartEl) auditBarPartEl.style.width = `${pPart}%`;
    if (auditBarPlanEl) auditBarPlanEl.style.width = `${pPlan}%`;

    if (roadmapPartADomainsEl && Array.isArray(data.partADomains)) {
      roadmapPartADomainsEl.textContent = "";
      data.partADomains.forEach((dom) => {
        const div = document.createElement("div");
        div.className = "part-a-domain-item";
        const nameEl = document.createElement("span");
        nameEl.textContent = dom.name;
        const countEl = document.createElement("span");
        countEl.className = "part-a-domain-count";
        countEl.textContent = `${dom.implemented}/${dom.total} done`;
        div.appendChild(nameEl);
        div.appendChild(countEl);
        roadmapPartADomainsEl.appendChild(div);
      });
    }

    if (roadmapPartTabsEl && data.countsByPart) {
      roadmapPartTabsEl.textContent = "";
      const allBtn = document.createElement("button");
      allBtn.type = "button";
      allBtn.className = "tab-btn is-active";
      allBtn.dataset.roadmapPart = "ALL";
      allBtn.textContent = `All (${fmtNum(total)})`;
      allBtn.addEventListener("click", () => {
        roadmapState.part = "ALL";
        roadmapState.page = 1;
        loadRoadmapPage();
      });
      roadmapPartTabsEl.appendChild(allBtn);

      Object.values(data.countsByPart).forEach((partObj) => {
        roadmapState.partDescriptions[partObj.id] = `${partObj.title} (${partObj.range}) — ${partObj.description}`;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "tab-btn";
        btn.dataset.roadmapPart = partObj.id;
        btn.textContent = `${partObj.shortLabel} (${fmtNum(partObj.total)})`;
        btn.addEventListener("click", () => {
          roadmapState.part = partObj.id;
          roadmapState.page = 1;
          loadRoadmapPage();
        });
        roadmapPartTabsEl.appendChild(btn);
      });
    }

    if (roadmapSubsystemEl && Array.isArray(data.subsystems)) {
      roadmapSubsystemEl.textContent = "";
      const allOpt = document.createElement("option");
      allOpt.value = "ALL";
      allOpt.textContent = `All Subsystems & Domains (${data.subsystems.length})`;
      roadmapSubsystemEl.appendChild(allOpt);
      data.subsystems.forEach((sub) => {
        const opt = document.createElement("option");
        opt.value = sub.name;
        opt.textContent = `${sub.name} (${fmtNum(sub.total)})`;
        roadmapSubsystemEl.appendChild(opt);
      });
    }
  }

  function updateRoadmapControlsUI(data) {
    roadmapPartTabsEl?.querySelectorAll("[data-roadmap-part]").forEach((btn) => {
      const match = btn.dataset.roadmapPart === roadmapState.part;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });

    roadmapStatusTabs.forEach((btn) => {
      const match = btn.dataset.roadmapStatus === roadmapState.status;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });

    if (roadmapPartSummaryEl) {
      if (roadmapState.part === "ALL") {
        roadmapPartSummaryEl.textContent =
          "Showing all 10,150 items across Parts A–G. Filter by Part, Subsystem, Status, or keyword below.";
      } else {
        roadmapPartSummaryEl.textContent = roadmapState.partDescriptions[roadmapState.part] || "";
      }
    }

    const hasFilters =
      roadmapState.part !== "ALL" ||
      roadmapState.subsystem !== "ALL" ||
      roadmapState.status !== "all" ||
      roadmapState.q.trim() !== "";
    if (roadmapClearBtn) {
      roadmapClearBtn.style.display = hasFilters ? "inline-flex" : "none";
    }

    if (!data) return;
    roadmapState.totalPages = data.totalPages || 1;
    const filteredTotal = data.filteredTotal ?? 0;
    const page = data.page || 1;
    const startItem = filteredTotal === 0 ? 0 : (page - 1) * data.limit + 1;
    const endItem = Math.min(filteredTotal, page * data.limit);

    if (roadmapResultSummaryEl) {
      roadmapResultSummaryEl.textContent =
        filteredTotal === data.total
          ? `Showing ${fmtNum(startItem)}–${fmtNum(endItem)} of ${fmtNum(data.total)} proposals`
          : `Matched ${fmtNum(filteredTotal)} of ${fmtNum(data.total)} proposals`;
    }
    if (roadmapPageLabelEl) {
      roadmapPageLabelEl.textContent = `Page ${fmtNum(page)} of ${fmtNum(roadmapState.totalPages)}`;
    }
    if (roadmapFooterRangeEl) {
      roadmapFooterRangeEl.textContent =
        filteredTotal === 0
          ? "0 matching proposals"
          : `Showing items ${fmtNum(startItem)}–${fmtNum(endItem)} of ${fmtNum(filteredTotal)}`;
    }
    if (roadmapPrevBtn) roadmapPrevBtn.disabled = page <= 1;
    if (roadmapNextBtn) roadmapNextBtn.disabled = page >= roadmapState.totalPages;
    if (roadmapJumpInput) {
      roadmapJumpInput.max = String(roadmapState.totalPages);
      roadmapJumpInput.value = String(page);
    }
  }

  function renderRoadmapRows(data) {
    if (!roadmapRowsEl) return;
    roadmapRowsEl.textContent = "";
    const items = data?.items || [];
    if (items.length === 0) {
      const empty = document.createElement("div");
      empty.className = "roadmap-empty-state";
      const msg = document.createElement("p");
      msg.textContent = "No v1.0.0 upgrade proposals matched your current search and filter criteria.";
      const resetAction = document.createElement("button");
      resetAction.type = "button";
      resetAction.className = "btn btn-secondary";
      resetAction.textContent = "Reset Filters";
      resetAction.addEventListener("click", resetRoadmapFilters);
      empty.appendChild(msg);
      empty.appendChild(resetAction);
      roadmapRowsEl.appendChild(empty);
      return;
    }

    const frag = document.createDocumentFragment();
    items.forEach((item) => {
      const row = document.createElement("div");
      row.className = "roadmap-row";

      const idCol = document.createElement("div");
      idCol.className = "roadmap-col-id";
      idCol.textContent = `#${String(item.id).padStart(5, "0")}`;
      if (String(item.id).padStart(5, "0") !== item.sourceNumber) {
        const ref = document.createElement("span");
        ref.className = "roadmap-col-ref";
        ref.textContent = `txt ${item.sourceNumber}.`;
        idCol.appendChild(ref);
      }

      const subCol = document.createElement("div");
      subCol.className = "roadmap-col-sub";
      const partSpan = document.createElement("span");
      partSpan.className = "roadmap-col-part";
      partSpan.textContent = `Part ${item.part}`;
      subCol.appendChild(partSpan);
      subCol.appendChild(document.createTextNode(` · ${item.subsystem}`));

      const titleCol = document.createElement("div");
      titleCol.className = "roadmap-col-title";
      titleCol.textContent = item.title;
      if (item.tag) {
        const tagSpan = document.createElement("span");
        tagSpan.className = "roadmap-col-tag";
        tagSpan.textContent = `[${item.tag}]`;
        titleCol.appendChild(tagSpan);
      }

      const statusCol = document.createElement("div");
      statusCol.className = `roadmap-col-status is-${item.status}`;
      statusCol.textContent = formatStatusLabel(item.status);

      row.appendChild(idCol);
      row.appendChild(subCol);
      row.appendChild(titleCol);
      row.appendChild(statusCol);
      frag.appendChild(row);
    });
    roadmapRowsEl.appendChild(frag);
  }

  async function loadRoadmapPage() {
    if (roadmapRowsEl && !roadmapRowsEl.children.length) {
      roadmapRowsEl.innerHTML = '<div class="roadmap-empty-state">Loading proposals from DOG_v1.0.0_10150_Upgrade_Ideas.txt...</div>';
    }
    const params = new URLSearchParams({
      q: roadmapState.q.trim(),
      part: roadmapState.part,
      subsystem: roadmapState.subsystem,
      status: roadmapState.status,
      page: String(roadmapState.page),
      limit: String(roadmapState.limit),
    });
    try {
      const res = await fetch(`/api/roadmap?${params.toString()}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      renderRoadmapMeta(data);
      updateRoadmapControlsUI(data);
      renderRoadmapRows(data);
    } catch (err) {
      if (roadmapRowsEl) {
        roadmapRowsEl.textContent = "";
        const box = document.createElement("div");
        box.className = "roadmap-empty-state";
        box.textContent = `Could not load roadmap catalogue (${err.message}). `;
        const retry = document.createElement("button");
        retry.type = "button";
        retry.className = "btn btn-secondary";
        retry.textContent = "Retry";
        retry.addEventListener("click", loadRoadmapPage);
        box.appendChild(retry);
        roadmapRowsEl.appendChild(box);
      }
    }
  }

  function resetRoadmapFilters() {
    roadmapState.part = "ALL";
    roadmapState.subsystem = "ALL";
    roadmapState.status = "all";
    roadmapState.q = "";
    roadmapState.page = 1;
    if (roadmapSearchEl) roadmapSearchEl.value = "";
    if (roadmapSubsystemEl) roadmapSubsystemEl.value = "ALL";
    loadRoadmapPage();
  }

  roadmapSearchEl?.addEventListener("input", () => {
    clearTimeout(roadmapDebounceTimer);
    roadmapDebounceTimer = setTimeout(() => {
      roadmapState.q = roadmapSearchEl.value;
      roadmapState.page = 1;
      loadRoadmapPage();
    }, 180);
  });

  roadmapSubsystemEl?.addEventListener("change", () => {
    roadmapState.subsystem = roadmapSubsystemEl.value;
    roadmapState.page = 1;
    loadRoadmapPage();
  });

  roadmapStatusTabs.forEach((btn) => {
    btn.addEventListener("click", () => {
      roadmapState.status = btn.dataset.roadmapStatus;
      roadmapState.page = 1;
      loadRoadmapPage();
    });
  });

  roadmapClearBtn?.addEventListener("click", resetRoadmapFilters);

  roadmapPrevBtn?.addEventListener("click", () => {
    if (roadmapState.page > 1) {
      roadmapState.page -= 1;
      loadRoadmapPage();
    }
  });

  roadmapNextBtn?.addEventListener("click", () => {
    if (roadmapState.page < roadmapState.totalPages) {
      roadmapState.page += 1;
      loadRoadmapPage();
    }
  });

  roadmapJumpBtn?.addEventListener("click", () => {
    const target = Number(roadmapJumpInput?.value || 1);
    if (Number.isFinite(target)) {
      roadmapState.page = Math.min(Math.max(Math.trunc(target), 1), roadmapState.totalPages);
      loadRoadmapPage();
    }
  });

  roadmapJumpInput?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      roadmapJumpBtn?.click();
    }
  });

  const readinessSummaryEl = document.getElementById("readiness-summary-label");
  const readinessChecksEl = document.getElementById("readiness-checks-list");
  const readinessSbomMetaEl = document.getElementById("readiness-sbom-meta");
  const readinessMatrixEl = document.getElementById("readiness-matrix-list");

  async function loadReadinessGate() {
    if (!readinessChecksEl) return;
    try {
      const res = await fetch("/api/readiness");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();

      if (readinessSummaryEl) {
        readinessSummaryEl.textContent = `${data.passedCount}/${data.totalChecks} release checks passed for v${data.version} (${data.gatePassed ? "v1.0.0 Release Gate Green" : "Attention Needed"})`;
      }

      readinessChecksEl.textContent = "";
      (data.checks || []).forEach((chk) => {
        const row = document.createElement("div");
        row.className = `readiness-check-row ${chk.passed ? "is-ok" : "is-err"}`;
        const left = document.createElement("div");
        const title = document.createElement("div");
        title.className = "readiness-check-title";
        title.textContent = chk.label;
        const detail = document.createElement("div");
        detail.className = "readiness-check-detail";
        detail.textContent = chk.detail;
        left.appendChild(title);
        left.appendChild(detail);

        const badge = document.createElement("span");
        badge.className = `readiness-check-status ${chk.passed ? "is-ok" : "is-err"}`;
        badge.textContent = chk.passed ? "PASS" : "FAIL";
        row.appendChild(left);
        row.appendChild(badge);
        readinessChecksEl.appendChild(row);
      });

      if (readinessSbomMetaEl && data.sbom) {
        const s = data.sbom;
        readinessSbomMetaEl.textContent = `${s.specFormat} · ${s.packageName} v${s.packageVersion} · ${s.license} · ${s.externalRustCrates} Rust crates · ${s.unsafeBlocksInCore} unsafe blocks · ${s.rustModuleInventoryCount} core modules`;
      }

      if (readinessMatrixEl && Array.isArray(data.compatibilityMatrix)) {
        readinessMatrixEl.textContent = "";
        data.compatibilityMatrix.forEach((target) => {
          const row = document.createElement("div");
          row.className = "readiness-matrix-row";
          const osEl = document.createElement("span");
          osEl.textContent = `${target.os} (${target.arch})`;
          const pkgEl = document.createElement("span");
          pkgEl.className = "readiness-matrix-pkg";
          pkgEl.textContent = `${target.tier} · ${target.packaging}`;
          row.appendChild(osEl);
          row.appendChild(pkgEl);
          readinessMatrixEl.appendChild(row);
        });
      }
    } catch (err) {
      if (readinessSummaryEl) readinessSummaryEl.textContent = `Could not load readiness report (${err.message}).`;
    }
  }

  // Initialize all sections
  const urlParams = new URLSearchParams(window.location.search);
  const queryCode = urlParams.get("code");
  const queryPreset = urlParams.get("preset");

  renderSyntaxShowcase();
  if (queryCode && editorTextarea) {
    editorTextarea.value = queryCode;
    executePlayground(false);
  } else if (queryPreset) {
    loadPreset(queryPreset, false);
  } else {
    loadPreset("english_tour", false);
  }
  renderWebFileEditor();
  loadWebPreset(WEB_STUDIO_PRESETS[0].id, false);
  renderStdlib();
  renderInstall();
  initWorkspaceBrowser();
  loadReadinessGate();
  loadRoadmapPage();

  // Record real visit + fetch full live stats + maintain heartbeat while visible
  recordSiteEvent("visit").then(fetchStatsSnapshot);
  setInterval(() => {
    if (document.visibilityState === "visible") {
      recordSiteEvent("heartbeat");
    }
  }, 60_000);
});
