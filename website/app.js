import {
  CLI_COMMANDS,
  INSTALL_METHODS,
  PLAYGROUND_PRESETS,
  STD_MODULES,
  SYNTAX_SHOWCASE,
} from "./data.js";
import { runDogSandbox } from "./sandbox.js";

const KEYWORDS = new Set([
  "let", "var", "const", "set", "to", "increase", "decrease", "by",
  "if", "then", "otherwise", "else", "elif", "unless", "match", "case",
  "try", "catch", "struct", "enum", "use", "end", "or", "and", "not",
  "is", "equals", "equal", "greater", "less", "than", "at", "least",
  "most", "over", "under", "fn", "define", "function", "returns",
  "return", "while", "until", "repeat", "times", "for", "each", "from",
  "in", "break", "continue", "true", "false",
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

  // Image fallback handlers
  document.querySelectorAll("img[data-fallback-id]").forEach((img) => {
    img.addEventListener("error", () => {
      img.style.display = "none";
      const fb = document.getElementById(img.dataset.fallbackId);
      if (fb) fb.style.display = "flex";
    });
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
    syntaxFileEl.textContent = `${item.file} (${activeDialect === "english" ? "English style" : "Classic style"})`;
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
      outputStatus.className = "output-status is-ok";
      outputStatus.textContent = `Passed · ${lastSandboxResult.steps} steps · ${lastSandboxResult.elapsedMs} ms`;
      if (activeOutputTab === "console") {
        outputBody.textContent =
          lastSandboxResult.output.length > 0
            ? lastSandboxResult.output.join("\n")
            : "(Program completed with no console output.)";
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
      } else {
        outputBody.textContent = lastSandboxResult.astSummary.join("\n") || "Stopped before type trace completion.";
      }
    }
  }

  function executePlayground() {
    lastSandboxResult = runDogSandbox(editorTextarea.value);
    renderPlaygroundOutput();
  }

  function loadPreset(id) {
    const preset = PLAYGROUND_PRESETS.find((p) => p.id === id) || PLAYGROUND_PRESETS[0];
    presetSelect.value = preset.id;
    editorTextarea.value = preset.code;
    executePlayground();
  }

  presetSelect.addEventListener("change", () => loadPreset(presetSelect.value));
  runBtn.addEventListener("click", executePlayground);
  resetBtn.addEventListener("click", () => loadPreset(presetSelect.value));
  outputTabBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      activeOutputTab = btn.dataset.outputTab;
      renderPlaygroundOutput();
    });
  });

  openInPlaygroundBtn?.addEventListener("click", () => {
    editorTextarea.value = getActiveSyntaxCode();
    executePlayground();
    document.getElementById("playground")?.scrollIntoView({ behavior: "smooth" });
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
      stdSummaryEl.textContent = `${rows.length} matching exports across 10 std.* modules for "${stdQuery.trim()}"`;
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
    docTitleEl.textContent = `${kind === "doc" ? "docs" : "examples"}/${name}`;
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

  // Initialize all sections
  renderSyntaxShowcase();
  loadPreset("english_tour");
  renderStdlib();
  renderInstall();
  initWorkspaceBrowser();
});
