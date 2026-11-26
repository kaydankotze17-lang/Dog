import {
  CHEATSHEET_CATEGORIES,
  CHEATSHEET_ENTRIES,
  ENGLISH_VS_CLASSIC_TABLE,
  OPERATOR_PRECEDENCE_TABLE,
  filterCheatSheetEntries,
  formatCheatSheetMarkdown,
} from "./cheatsheet_engine.mjs";

const KEYWORDS = new Set([
  "let", "var", "const", "set", "to", "increase", "decrease", "by",
  "if", "then", "otherwise", "else", "elif", "unless", "match", "case",
  "try", "catch", "struct", "enum", "use", "style", "vars", "rule",
  "animate", "extend", "on", "end", "or", "and", "not", "is", "equals",
  "equal", "greater", "less", "than", "at", "least", "most", "over",
  "under", "fn", "define", "function", "returns", "return", "while",
  "until", "repeat", "times", "for", "each", "from", "in", "break",
  "continue", "true", "false", "type",
  "keep", "make", "do", "give", "show", "when", "loop", "stop", "skip",
  "oops", "bump", "add", "take", "check", "null",
]);

const TYPES = new Set(["text", "number", "decimal", "bool", "void", "list", "map", "fn"]);

function renderHighlightedCode(container, code) {
  container.textContent = "";
  const lines = String(code || "").split("\n");
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
    }, 1500);
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

  let activeCategory = "all";
  let searchQuery = "";

  const searchInput = document.getElementById("cheatsheet-search");
  const categoryTabsEl = document.getElementById("cheatsheet-category-tabs");
  const countEl = document.getElementById("cheatsheet-count");
  const resetBtn = document.getElementById("cheatsheet-reset-btn");
  const cardsEl = document.getElementById("cheatsheet-cards");
  const printBtn = document.getElementById("cheatsheet-print-btn");
  const downloadMdBtn = document.getElementById("cheatsheet-download-md-btn");
  const dialectTbody = document.getElementById("dialect-table-body");
  const precedenceTbody = document.getElementById("precedence-table-body");

  // Render category tabs
  if (categoryTabsEl) {
    CHEATSHEET_CATEGORIES.forEach((cat) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "tab-btn";
      btn.dataset.csCategory = cat.id;
      btn.setAttribute("role", "tab");
      btn.textContent = cat.label;
      btn.addEventListener("click", () => {
        activeCategory = cat.id;
        renderCards();
      });
      categoryTabsEl.appendChild(btn);
    });
  }

  function renderCards() {
    const filtered = filterCheatSheetEntries(CHEATSHEET_ENTRIES, {
      category: activeCategory,
      q: searchQuery,
    });

    categoryTabsEl?.querySelectorAll("[data-cs-category]").forEach((btn) => {
      const match = btn.dataset.csCategory === activeCategory;
      btn.classList.toggle("is-active", match);
      btn.setAttribute("aria-selected", String(match));
    });

    if (countEl) {
      countEl.textContent = `${filtered.length} of ${CHEATSHEET_ENTRIES.length} topics`;
    }

    const isFiltered = activeCategory !== "all" || searchQuery.trim() !== "";
    resetBtn?.classList.toggle("is-hidden", !isFiltered);

    if (!cardsEl) return;
    cardsEl.textContent = "";

    if (filtered.length === 0) {
      const empty = document.createElement("div");
      empty.className = "cheatsheet-empty";
      const msg = document.createElement("p");
      msg.textContent = "No cheat sheet cards matched your filter.";
      const clr = document.createElement("button");
      clr.type = "button";
      clr.className = "btn btn-secondary";
      clr.textContent = "Reset Search & Filters";
      clr.addEventListener("click", resetFilters);
      empty.appendChild(msg);
      empty.appendChild(clr);
      cardsEl.appendChild(empty);
      return;
    }

    const frag = document.createDocumentFragment();
    filtered.forEach((entry) => {
      const card = document.createElement("article");
      card.className = "cheatsheet-card";
      card.id = `cs-${entry.id}`;

      const head = document.createElement("div");
      head.className = "cheatsheet-card-head";

      const title = document.createElement("h2");
      title.className = "cheatsheet-card-title";
      title.textContent = entry.title;

      const meta = document.createElement("span");
      meta.className = "cheatsheet-card-tag";
      meta.textContent = entry.syntaxBadge || entry.category;

      head.appendChild(title);
      head.appendChild(meta);

      const summary = document.createElement("p");
      summary.className = "cheatsheet-card-summary";
      summary.textContent = entry.summary;

      const panel = document.createElement("div");
      panel.className = "code-panel";

      const bar = document.createElement("div");
      bar.className = "code-panel-bar";

      const label = document.createElement("span");
      label.textContent = entry.category === "web-dgss" ? "DOG Web / .dgss" : entry.category === "cli" ? "Shell / CLI" : `${entry.id}.dg`;

      const actions = document.createElement("div");
      actions.className = "code-panel-actions";

      if (entry.playgroundRunnable) {
        const tryLink = document.createElement("a");
        tryLink.className = "code-action-btn";
        tryLink.href = `/?code=${encodeURIComponent(entry.code)}#playground`;
        tryLink.textContent = "Try in Playground";
        actions.appendChild(tryLink);
      }

      const copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "code-action-btn";
      copyBtn.textContent = "Copy";
      copyBtn.addEventListener("click", () => copyWithFeedback(copyBtn, entry.code));
      actions.appendChild(copyBtn);

      bar.appendChild(label);
      bar.appendChild(actions);

      const pre = document.createElement("pre");
      pre.className = "code-pre";
      renderHighlightedCode(pre, entry.code);

      panel.appendChild(bar);
      panel.appendChild(pre);

      card.appendChild(head);
      card.appendChild(summary);
      card.appendChild(panel);
      frag.appendChild(card);
    });
    cardsEl.appendChild(frag);
  }

  function resetFilters() {
    activeCategory = "all";
    searchQuery = "";
    if (searchInput) searchInput.value = "";
    renderCards();
  }

  searchInput?.addEventListener("input", () => {
    searchQuery = searchInput.value;
    renderCards();
  });

  resetBtn?.addEventListener("click", resetFilters);

  printBtn?.addEventListener("click", () => {
    window.print();
  });

  downloadMdBtn?.addEventListener("click", () => {
    const md = formatCheatSheetMarkdown(
      CHEATSHEET_ENTRIES,
      ENGLISH_VS_CLASSIC_TABLE,
      OPERATOR_PRECEDENCE_TABLE
    );
    const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "DOG_CHEAT_SHEET.md";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  });

  // Render dialect comparison table
  if (dialectTbody) {
    ENGLISH_VS_CLASSIC_TABLE.forEach((row) => {
      const tr = document.createElement("tr");
      const tdConcept = document.createElement("td");
      tdConcept.className = "cs-cell-concept";
      tdConcept.textContent = row.concept;

      const tdEng = document.createElement("td");
      const codeEng = document.createElement("code");
      codeEng.className = "cs-inline-block";
      codeEng.textContent = row.english;
      tdEng.appendChild(codeEng);

      const tdCls = document.createElement("td");
      const codeCls = document.createElement("code");
      codeCls.className = "cs-inline-block";
      codeCls.textContent = row.classic;
      tdCls.appendChild(codeCls);

      const tdSmp = document.createElement("td");
      const codeSmp = document.createElement("code");
      codeSmp.className = "cs-inline-block";
      codeSmp.textContent = row.simple || "";
      tdSmp.appendChild(codeSmp);

      tr.appendChild(tdConcept);
      tr.appendChild(tdEng);
      tr.appendChild(tdCls);
      tr.appendChild(tdSmp);
      dialectTbody.appendChild(tr);
    });
  }

  // Render operator precedence table
  if (precedenceTbody) {
    OPERATOR_PRECEDENCE_TABLE.forEach((row) => {
      const tr = document.createElement("tr");
      const tdLvl = document.createElement("td");
      tdLvl.className = "cs-cell-concept";
      tdLvl.textContent = row.level;

      const tdOps = document.createElement("td");
      const codeOps = document.createElement("code");
      codeOps.textContent = row.operators;
      tdOps.appendChild(codeOps);

      const tdDesc = document.createElement("td");
      tdDesc.textContent = row.description;

      tr.appendChild(tdLvl);
      tr.appendChild(tdOps);
      tr.appendChild(tdDesc);
      precedenceTbody.appendChild(tr);
    });
  }

  renderCards();
});
