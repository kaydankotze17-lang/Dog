// Pure DOG Web (.dg -> JavaScript) + DOG CSS (.dgss -> CSS) Compiler
// Shared between the official website's Web & CSS Studio and automated node:test suites.

export class DogWebDiagnostic extends Error {
  constructor({ code, title, line, why, fix, severity = "error" }) {
    super(`[${code}] line ${line || 1}: ${title}`);
    this.code = code || "E004";
    this.severity = severity;
    this.title = title;
    this.line = line || 1;
    this.why = why || "";
    this.fix = fix || "";
  }

  toJSON() {
    return {
      code: this.code,
      severity: this.severity,
      line: this.line,
      title: this.title,
      why: this.why,
      fix: this.fix,
    };
  }

  format() {
    return [
      `[${this.code}] line ${this.line}: ${this.title}`,
      this.why ? `WHY: ${this.why}` : "",
      this.fix ? `FIX: ${this.fix}` : "",
    ]
      .filter(Boolean)
      .join("\n");
  }
}

function expandVarRefs(val) {
  return String(val).replace(/\$([a-zA-Z_][a-zA-Z0-9_-]*)/g, "var(--$1)");
}

export function expandDogCssDeclaration(rawKey, rawVal) {
  const prop = String(rawKey).trim().toLowerCase();
  const val = expandVarRefs(String(rawVal).trim());
  switch (prop) {
    case "bg":
      return [["background", val]];
    case "ink":
    case "fg":
      return [["color", val]];
    case "pad":
    case "p":
      return [["padding", val]];
    case "px":
      return [["padding-inline", val]];
    case "py":
      return [["padding-block", val]];
    case "m":
      return [["margin", val]];
    case "mx":
      return [["margin-inline", val]];
    case "my":
      return [["margin-block", val]];
    case "w":
      return [["width", val]];
    case "h":
      return [["height", val]];
    case "max-w":
      return [["max-width", val]];
    case "min-h":
      return [["min-height", val]];
    case "radius":
    case "round":
      return [["border-radius", val]];
    case "size":
      return [["font-size", val]];
    case "weight":
      return [["font-weight", val]];
    case "font":
      return [["font-family", val]];
    case "leading":
      return [["line-height", val]];
    case "ease":
      return [["transition", val]];
    case "align":
      return [["align-items", val]];
    case "justify":
      return [["justify-content", val]];
    case "cols":
      return [
        ["display", "grid"],
        ["grid-template-columns", /^\d+$/.test(val) ? `repeat(${val}, minmax(0, 1fr))` : val],
      ];
    case "grid":
      return [
        ["display", "grid"],
        ["grid-template-columns", val],
      ];
    case "flex": {
      const lower = val.toLowerCase();
      if (lower === "row") {
        return [
          ["display", "flex"],
          ["flex-direction", "row"],
        ];
      }
      if (lower === "col" || lower === "column") {
        return [
          ["display", "flex"],
          ["flex-direction", "column"],
        ];
      }
      return [["flex", val]];
    }
    case "center":
      if (["true", "all", "yes"].includes(val.toLowerCase())) {
        return [
          ["display", "flex"],
          ["align-items", "center"],
          ["justify-content", "center"],
        ];
      }
      return [["text-align", "center"]];
    case "shadow": {
      const presets = {
        sm: "0 1px 3px rgba(24, 26, 31, 0.08)",
        md: "0 6px 20px rgba(24, 26, 31, 0.08)",
        lg: "0 16px 36px rgba(24, 26, 31, 0.12)",
      };
      return [["box-shadow", presets[val.toLowerCase()] || val]];
    }
    case "user-select":
      return [["-webkit-user-select", val], ["user-select", val]];
    case "backdrop-filter":
      return [["-webkit-backdrop-filter", val], ["backdrop-filter", val]];
    case "appearance":
      return [["-webkit-appearance", val], ["appearance", val]];
    default:
      return [[prop, val]];
  }
}

export function minifyDogCss(cssText) {
  return String(cssText || "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\s*{\s*/g, "{")
    .replace(/\s*}\s*/g, "}")
    .replace(/\s*:\s*/g, ":")
    .replace(/\s*;\s*/g, ";")
    .replace(/;}/g, "}")
    .replace(/\n+/g, "")
    .trim();
}

export function treeShakeDogCss(cssText, usageText = "") {
  const hay = String(usageText || "");
  const prunedSelectors = [];
  const keptSelectors = [];

  function selectorUsed(selector) {
    const clean = selector.trim();
    if (!clean || clean === ":root" || clean === "*" || clean === "html" || clean === "body") return true;
    // Always retain interactive pseudo/tag rules if base class or tag appears
    const classes = [...clean.matchAll(/\.([a-zA-Z_][a-zA-Z0-9_-]*)/g)].map((m) => m[1]);
    const ids = [...clean.matchAll(/#([a-zA-Z_][a-zA-Z0-9_-]*)/g)].map((m) => m[1]);
    if (classes.length === 0 && ids.length === 0) return true;
    const hit =
      classes.some((c) => hay.includes(c)) ||
      ids.some((id) => hay.includes(id));
    return hit;
  }

  function processRuleBlocks(src) {
    const kept = [];
    let i = 0;
    while (i < src.length) {
      while (i < src.length && /\s/.test(src[i])) i++;
      if (i >= src.length) break;
      const brace = src.indexOf("{", i);
      if (brace < 0) {
        kept.push(src.slice(i));
        break;
      }
      const header = src.slice(i, brace).trim();
      let depth = 1;
      let j = brace + 1;
      while (j < src.length && depth > 0) {
        if (src[j] === "{") depth++;
        else if (src[j] === "}") depth--;
        j++;
      }
      const body = src.slice(brace + 1, j - 1);
      if (header.startsWith("@keyframes")) {
        kept.push(`${header} {${body}}`);
      } else if (header.startsWith("@media")) {
        const inner = processRuleBlocks(body);
        if (inner.trim()) kept.push(`${header} {\n${inner}\n}`);
      } else {
        const subSelectors = header.split(",").map((s) => s.trim()).filter(Boolean);
        const retained = subSelectors.filter((s) => selectorUsed(s));
        const dropped = subSelectors.filter((s) => !selectorUsed(s));
        prunedSelectors.push(...dropped);
        if (retained.length > 0) {
          keptSelectors.push(...retained);
          kept.push(`${retained.join(", ")} {${body}}`);
        }
      }
      i = j;
    }
    return kept.join("\n\n");
  }

  const shaken = processRuleBlocks(String(cssText || ""));
  return { css: shaken, prunedSelectors, keptSelectors };
}

export function analyzeWebCompatibilityAndA11y({ code = "", css = "", userJs = "", prunedCount = 0 } = {}) {
  const diags = [];
  if (/-webkit-(backdrop-filter|user-select|appearance)/i.test(css)) {
    diags.push({
      code: "I001",
      severity: "info",
      line: 1,
      title: "Safari/WebKit vendor prefixes applied automatically (-webkit-*).",
      fix: "No action required.",
    });
  }
  if (prunedCount > 0) {
    diags.push({
      code: "I002",
      severity: "info",
      line: 1,
      title: `CSS tree-shaking removed ${prunedCount} unused selector(s).`,
      fix: "Toggle Tree-Shake CSS off if selectors are added dynamically outside rendered templates.",
    });
  }
  if (/<img\b(?![^>]*\balt=)/i.test(code) || /html_el\("img"/.test(code) && !/\"alt\"/.test(code)) {
    diags.push({
      code: "W101",
      severity: "warning",
      line: 1,
      title: "Accessibility check: <img> element detected without 'alt' text attribute.",
      fix: 'Add `"alt": "Description"` to `html_el("img", ...)` attributes.',
    });
  }
  if (/<input\b/i.test(userJs) && !/aria-label|<label\b/i.test(userJs) && !/webField|webInput/.test(userJs)) {
    diags.push({
      code: "W102",
      severity: "warning",
      line: 1,
      title: "Accessibility check: raw <input> lacks an associated <label> or aria-label.",
      fix: "Prefer `web_field(id, label, placeholder, type)` for automatic accessible `<label>` pairing.",
    });
  }
  if (/<(button|a)\b(?![^>]*aria-label=)[^>]*>\s*<\/\1>/i.test(code) || /<(button|a)\b(?![^>]*aria-label=)[^>]*>\s*<\/\1>/i.test(userJs)) {
    diags.push({
      code: "W103",
      severity: "warning",
      line: 1,
      title: "Accessibility check: empty <button> or <a> detected without visible text or aria-label.",
      fix: 'Add descriptive button/link text or `"aria-label": "..."` attribute.',
    });
  }
  return diags;
}

function stripDgssComment(line) {
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === "'" && !inDouble) inSingle = !inSingle;
    else if (ch === '"' && !inSingle) inDouble = !inDouble;
    else if (!inSingle && !inDouble) {
      if (ch === "/" && line[i + 1] === "/") return line.slice(0, i);
      if (ch === "#") {
        const rest = line.slice(i + 1);
        const looksHex = /^[0-9a-fA-F]/.test(rest) && !rest.startsWith(" ");
        if (i === 0 || (/\s/.test(line[i - 1]) && !looksHex)) {
          return line.slice(0, i);
        }
      }
    }
  }
  return line;
}

function renderMediaBlock(query, innerCss) {
  const map = {
    mobile: "(max-width: 768px)",
    tablet: "(max-width: 1024px)",
    desktop: "(min-width: 1025px)",
    dark: "(prefers-color-scheme: dark)",
  };
  const q = map[query.trim().toLowerCase()] || (query.trim().startsWith("(") ? query.trim() : `(${query.trim()})`);
  const indented = innerCss
    .split("\n")
    .map((l) => (l.trim() ? `  ${l}` : ""))
    .join("\n");
  return `@media ${q} {\n${indented}\n}`;
}

export function compileDogCss(dsl, options = {}) {
  const lines = String(dsl || "").split(/\r?\n/);
  const stack = [];
  const roots = [];

  for (let idx = 0; idx < lines.length; idx++) {
    const lineNo = idx + 1;
    const trimmed = stripDgssComment(lines[idx]).trim();
    if (!trimmed) continue;

    if (trimmed === "end" || trimmed === "}") {
      const finished = stack.pop();
      if (!finished) {
        throw new DogWebDiagnostic({
          code: "E005",
          title: "Unexpected `end` in DOG CSS stylesheet.",
          line: lineNo,
          why: "Found `end` with no matching open block.",
          fix: "Remove the extra `end` or open a `rule`, `vars`, `on`, or `animate` block.",
        });
      }
      const parent = stack[stack.length - 1];
      if (!parent) {
        roots.push(finished);
      } else if (parent.kind === "rule" && (finished.kind === "state" || finished.kind === "rule")) {
        parent.children.push(finished);
      } else if ((parent.kind === "media" || parent.kind === "state") && (finished.kind === "rule" || finished.kind === "vars")) {
        parent.rules.push(finished);
      } else if (parent.kind === "animate" && finished.kind === "keystop") {
        parent.stops.push(finished);
      } else {
        throw new DogWebDiagnostic({
          code: "E004",
          title: `Invalid nested block inside \`${parent.kind}\`.`,
          line: lineNo,
          why: "This DOG CSS block cannot be nested directly here.",
          fix: "Nest `on hover` inside `rule`, and `rule` inside `on mobile` / `media`.",
        });
      }
      continue;
    }

    const openBody = trimmed.replace(/\{\s*$/, "").trim();
    if (openBody === "vars") {
      stack.push({ kind: "vars", line: lineNo, entries: [] });
      continue;
    }
    if (openBody.startsWith("rule ")) {
      const selector = openBody.slice(5).trim();
      if (!selector) {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected CSS selector after `rule`.",
          line: lineNo,
          why: "Missing target selector for rule block.",
          fix: "Example: `rule .card`",
        });
      }
      stack.push({ kind: "rule", selector, line: lineNo, decls: [], children: [] });
      continue;
    }
    if (openBody.startsWith("on ")) {
      const trigger = openBody.slice(3).trim();
      if (!trigger) {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected state or breakpoint after `on`.",
          line: lineNo,
          why: "Missing state/breakpoint name.",
          fix: "Use e.g. `on hover` or `on mobile`.",
        });
      }
      stack.push({ kind: "state", trigger, line: lineNo, decls: [], rules: [] });
      continue;
    }
    if (openBody.startsWith("media ")) {
      const query = openBody.slice(6).trim();
      stack.push({ kind: "media", query, line: lineNo, rules: [] });
      continue;
    }
    if (openBody.startsWith("animate ")) {
      const name = openBody.slice(8).trim();
      stack.push({ kind: "animate", name, line: lineNo, stops: [] });
      continue;
    }
    const top = stack[stack.length - 1];
    if (top && top.kind === "animate" && (openBody === "from" || openBody === "to" || /^\d+%$/.test(openBody))) {
      stack.push({ kind: "keystop", stop: openBody, line: lineNo, decls: [] });
      continue;
    }

    const declClean = trimmed.replace(/;\s*$/, "").trim();
    const colonIdx = declClean.indexOf(":");
    const eqIdx = declClean.indexOf("=");
    const splitAt = colonIdx > 0 ? colonIdx : eqIdx > 0 ? eqIdx : -1;
    if (splitAt <= 0) {
      throw new DogWebDiagnostic({
        code: "E004",
        title: `Unrecognized DOG CSS line: \`${trimmed}\``,
        line: lineNo,
        why: "Expected `property: value` or a DOG CSS block (`vars`, `rule`, `on`, `media`, `animate`).",
        fix: "Write `prop: value` inside a `rule`, `vars`, or `on` block.",
      });
    }
    if (!top) {
      throw new DogWebDiagnostic({
        code: "E004",
        title: "Property declaration outside any block.",
        line: lineNo,
        why: `\`${trimmed}\` must live inside \`vars\`, \`rule\`, or \`on\`.`,
        fix: "Wrap declarations inside `rule .card ... end` or `vars ... end`.",
      });
    }
    const k = declClean.slice(0, splitAt).trim();
    const v = declClean.slice(splitAt + 1).trim();
    if (top.kind === "vars") top.entries.push([k, v]);
    else if (top.kind === "rule" || top.kind === "state" || top.kind === "keystop") top.decls.push([k, v]);
    else {
      throw new DogWebDiagnostic({
        code: "E004",
        title: `Property \`${k}\` cannot appear directly inside \`${top.kind}\`.`,
        line: lineNo,
        why: "Open a nested `rule` or stop (`from` / `to`) first.",
        fix: "Add `rule .selector ... end` inside media blocks.",
      });
    }
  }

  if (stack.length > 0) {
    const unclosed = stack[stack.length - 1];
    throw new DogWebDiagnostic({
      code: "E005",
      title: `Unclosed DOG CSS \`${unclosed.kind}\` block (missing \`end\`).`,
      line: unclosed.line,
      why: "Every `vars`, `rule`, `on`, `media`, and `animate` block must end with `end`.",
      fix: "Add a closing `end` line.",
    });
  }

  const out = [];
  const ruleDeclsBySelector = new Map();
  function emitBlock(block, parentSel = null) {
    if (block.kind === "vars") {
      if (block.entries.length > 0) {
        const linesOut = block.entries.map(
          ([k, v]) => `  --${k.replace(/^-+/, "")}: ${expandVarRefs(v)};`
        );
        out.push(`:root {\n${linesOut.join("\n")}\n}`);
      }
      return;
    }
    if (block.kind === "rule") {
      const fullSel = parentSel
        ? block.selector.includes("&")
          ? block.selector.replace(/&/g, parentSel)
          : `${parentSel} ${block.selector}`
        : block.selector;
      // Support `extend: .base-card` theme/style inheritance inside rules
      const resolvedDecls = [];
      for (const [k, v] of block.decls) {
        if (k.toLowerCase() === "extend" && ruleDeclsBySelector.has(v.trim())) {
          resolvedDecls.push(...ruleDeclsBySelector.get(v.trim()));
        } else {
          resolvedDecls.push([k, v]);
        }
      }
      ruleDeclsBySelector.set(fullSel, resolvedDecls);
      if (resolvedDecls.length > 0) {
        const declLines = [];
        for (const [k, v] of resolvedDecls) {
          for (const [ek, ev] of expandDogCssDeclaration(k, v)) {
            declLines.push(`  ${ek}: ${ev};`);
          }
        }
        out.push(`${fullSel} {\n${declLines.join("\n")}\n}`);
      }
      for (const ch of block.children) emitBlock(ch, fullSel);
      return;
    }
    if (block.kind === "state") {
      const trig = block.trigger.toLowerCase();
      if (["mobile", "tablet", "desktop", "dark"].includes(trig)) {
        const inner = [];
        if (parentSel && block.decls.length > 0) {
          const declLines = [];
          for (const [k, v] of block.decls) {
            for (const [ek, ev] of expandDogCssDeclaration(k, v)) {
              declLines.push(`  ${ek}: ${ev};`);
            }
          }
          inner.push(`${parentSel} {\n${declLines.join("\n")}\n}`);
        }
        const savedLen = out.length;
        for (const r of block.rules) emitBlock(r, null);
        const childCss = out.splice(savedLen);
        inner.push(...childCss);
        out.push(renderMediaBlock(trig, inner.join("\n\n")));
      } else if (parentSel) {
        if (block.decls.length > 0) {
          const declLines = [];
          for (const [k, v] of block.decls) {
            for (const [ek, ev] of expandDogCssDeclaration(k, v)) {
              declLines.push(`  ${ek}: ${ev};`);
            }
          }
          out.push(`${parentSel}:${trig} {\n${declLines.join("\n")}\n}`);
        }
      } else {
        throw new DogWebDiagnostic({
          code: "E004",
          title: `\`on ${block.trigger}\` must be nested inside a \`rule\` block.`,
          line: block.line,
          why: "Pseudo-states attach to a parent selector.",
          fix: "Place `on hover ... end` inside `rule .my-btn ... end`, or use `on mobile ... end` at top level.",
        });
      }
      return;
    }
    if (block.kind === "media") {
      const savedLen = out.length;
      for (const r of block.rules) emitBlock(r, null);
      const inner = out.splice(savedLen);
      out.push(renderMediaBlock(block.query, inner.join("\n\n")));
      return;
    }
    if (block.kind === "animate") {
      const stops = block.stops.map((st) => {
        const declLines = [];
        for (const [k, v] of st.decls) {
          for (const [ek, ev] of expandDogCssDeclaration(k, v)) {
            declLines.push(`    ${ek}: ${ev};`);
          }
        }
        return `  ${st.stop} {\n${declLines.join("\n")}\n  }`;
      });
      out.push(`@keyframes ${block.name} {\n${stops.join("\n")}\n}`);
    }
  }

  for (const b of roots) emitBlock(b);
  let fullCss = out.join("\n\n");
  if (options?.treeShake && options?.usageText !== undefined) {
    fullCss = treeShakeDogCss(fullCss, options.usageText).css;
  }
  return options?.minify ? minifyDogCss(fullCss) : fullCss;
}

// =========================================================================
// DOG (.dg) -> Browser JavaScript Transpiler for Interactive Websites
// =========================================================================

function stripDgComment(raw) {
  let inStr = false;
  let escape = false;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (escape) {
      escape = false;
      continue;
    }
    if (inStr) {
      if (ch === "\\") escape = true;
      else if (ch === '"') inStr = false;
      continue;
    }
    if (ch === '"') {
      inStr = true;
      continue;
    }
    if (ch === "#" || (ch === "/" && raw[i + 1] === "/")) return raw.slice(0, i);
  }
  return raw;
}

function splitTopLevelCommas(str) {
  const items = [];
  let depth = 0;
  let inStr = false;
  let escape = false;
  let start = 0;
  for (let i = 0; i < str.length; i++) {
    const c = str[i];
    if (escape) {
      escape = false;
      continue;
    }
    if (inStr) {
      if (c === "\\") escape = true;
      else if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') inStr = true;
    else if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") depth--;
    else if (c === "," && depth === 0) {
      items.push(str.slice(start, i).trim());
      start = i + 1;
    }
  }
  const tail = str.slice(start).trim();
  if (tail) items.push(tail);
  return items;
}

function tokenizeDgExpr(src, line) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    if (ch === "$" && src[i + 1] === '"') {
      i += 2;
      const parts = [];
      let lit = "";
      let closed = false;
      while (i < src.length) {
        const c = src[i];
        if (c === '"') {
          i++;
          closed = true;
          break;
        }
        if (c === "\\") {
          const next = src[i + 1];
          i += 2;
          if (next === "n") lit += "\n";
          else if (next === "t") lit += "\t";
          else if (next !== undefined) lit += next;
          continue;
        }
        if (c === "{" && src[i + 1] === "{") {
          lit += "{";
          i += 2;
          continue;
        }
        if (c === "}" && src[i + 1] === "}") {
          lit += "}";
          i += 2;
          continue;
        }
        if (c === "{") {
          if (lit) {
            parts.push({ type: "lit", value: lit });
            lit = "";
          }
          i++;
          let depth = 1;
          let exprStr = "";
          let innerStr = false;
          while (i < src.length && depth > 0) {
            const ec = src[i];
            if (innerStr) {
              exprStr += ec;
              if (ec === "\\") {
                exprStr += src[i + 1] || "";
                i += 2;
                continue;
              }
              if (ec === '"') innerStr = false;
              i++;
              continue;
            }
            if (ec === '"') {
              innerStr = true;
              exprStr += ec;
              i++;
              continue;
            }
            if (ec === "{") depth++;
            else if (ec === "}") {
              depth--;
              if (depth === 0) {
                i++;
                break;
              }
            }
            exprStr += ec;
            i++;
          }
          if (depth !== 0) {
            throw new DogWebDiagnostic({
              code: "E002",
              title: "Unclosed '{' inside interpolated string.",
              line,
              why: "Placeholder `{...}` was not closed.",
              fix: 'Close braces inside $"...".',
            });
          }
          parts.push({ type: "expr", value: exprStr.trim() });
          continue;
        }
        lit += c;
        i++;
      }
      if (!closed) {
        throw new DogWebDiagnostic({
          code: "E002",
          title: "Unterminated interpolated string.",
          line,
          why: 'Missing closing `"` on `$"..."`.',
          fix: 'Add closing quote `"` at the end of text.',
        });
      }
      if (lit) parts.push({ type: "lit", value: lit });
      tokens.push({ type: "interp", parts });
      continue;
    }
    if (ch === '"') {
      i++;
      let val = "";
      let closed = false;
      while (i < src.length) {
        const c = src[i];
        if (c === '"') {
          i++;
          closed = true;
          break;
        }
        if (c === "\\") {
          const next = src[i + 1];
          i += 2;
          if (next === "n") val += "\n";
          else if (next === "t") val += "\t";
          else if (next !== undefined) val += next;
          continue;
        }
        val += c;
        i++;
      }
      if (!closed) {
        throw new DogWebDiagnostic({
          code: "E002",
          title: "Unterminated string literal.",
          line,
          why: 'Missing closing `"`.',
          fix: 'Close the quote `"`.',
        });
      }
      tokens.push({ type: "str", value: val });
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let num = "";
      let isHex = false;
      let isBin = false;
      if (ch === "0" && (src[i + 1] === "x" || src[i + 1] === "X")) {
        isHex = true;
        num = "0x";
        i += 2;
        while (i < src.length && /[0-9a-fA-F_]/.test(src[i])) {
          if (src[i] !== "_") num += src[i];
          i++;
        }
        tokens.push({ type: "num", value: parseInt(num, 16), isDecimal: false });
        continue;
      }
      if (ch === "0" && (src[i + 1] === "b" || src[i + 1] === "B")) {
        isBin = true;
        i += 2;
        let bits = "";
        while (i < src.length && /[01_]/.test(src[i])) {
          if (src[i] !== "_") bits += src[i];
          i++;
        }
        tokens.push({ type: "num", value: parseInt(bits, 2), isDecimal: false });
        continue;
      }
      let hasDot = false;
      while (i < src.length && (/[0-9_]/.test(src[i]) || (src[i] === "." && !hasDot && /[0-9]/.test(src[i + 1] || "")))) {
        if (src[i] === ".") hasDot = true;
        if (src[i] !== "_") num += src[i];
        i++;
      }
      void isHex;
      void isBin;
      tokens.push({
        type: "num",
        value: hasDot ? Number(num) : parseInt(num, 10),
        isDecimal: hasDot,
      });
      continue;
    }
    const two = src.slice(i, i + 2);
    if (["==", "!=", ">=", "<=", "..", "=>", "|>", "??"].includes(two)) {
      tokens.push({ type: "op", value: two });
      i += 2;
      continue;
    }
    if (two === "&&") {
      throw new DogWebDiagnostic({
        code: "E001",
        title: "Use `and` instead of `&&`.",
        line,
        why: "DOG uses English `and` / `or` / `not` logical operators.",
        fix: "Replace `&&` with `and`.",
      });
    }
    if (two === "||") {
      throw new DogWebDiagnostic({
        code: "E001",
        title: "Use `or` instead of `||`.",
        line,
        why: "DOG uses English `and` / `or` / `not` logical operators.",
        fix: "Replace `||` with `or`.",
      });
    }
    if (ch === "!") {
      throw new DogWebDiagnostic({
        code: "E001",
        title: "Use `not` instead of `!`.",
        line,
        why: "DOG uses English `not`.",
        fix: "Replace `!cond` with `not cond`.",
      });
    }
    if ("+-*/%><()[]{}:,.".includes(ch)) {
      tokens.push({ type: "punct", value: ch });
      i++;
      continue;
    }
    if (/[a-zA-Z_]/.test(ch)) {
      let id = "";
      while (i < src.length && /[a-zA-Z0-9_]/.test(src[i])) {
        id += src[i];
        i++;
      }
      if (id === "true" || id === "false") {
        tokens.push({ type: "bool", value: id === "true" });
      } else if (id === "null") {
        tokens.push({ type: "null", value: null });
      } else {
        tokens.push({ type: "id", value: id });
      }
      continue;
    }
    throw new DogWebDiagnostic({
      code: "E001",
      title: `Unexpected character '${ch}' in DOG expression.`,
      line,
      why: `Character '${ch}' is not valid here.`,
      fix: "Remove or replace invalid syntax.",
    });
  }
  return tokens;
}

function inferRoughType(node, envTypes) {
  if (!node) return "unknown";
  if (node.kind === "str" || node.kind === "interp") return "text";
  if (node.kind === "num") return node.isDecimal ? "decimal" : "number";
  if (node.kind === "bool") return "bool";
  if (node.kind === "id") return envTypes.get(node.name) || "unknown";
  if (node.kind === "call") {
    if (["text", "lower", "upper", "trim", "join", "dog_css", "css_rule", "css_vars", "css_media", "css_keyframes", "css_theme", "css_bundle", "html_el", "html_page", "web_nav", "web_card", "web_grid", "web_button", "web_input", "web_badge", "web_alert", "web_field", "web_form", "web_render_component", "web_tabs", "web_modal", "web_table", "web_progress", "web_toast", "dom_attr", "dom_get_value", "web_storage_get", "web_route", "json_stringify", "html_escape"].includes(node.callee)) {
      return "text";
    }
    if (["number", "len", "round", "floor", "ceil"].includes(node.callee)) return "number";
    if (["contains", "starts_with", "ends_with", "regex_match", "dom_toggle_class", "web_validate_required", "web_validate_email"].includes(node.callee)) return "bool";
  }
  if (node.kind === "bin" && node.op === "+") {
    const l = inferRoughType(node.left, envTypes);
    const r = inferRoughType(node.right, envTypes);
    if ((l === "text" && (r === "number" || r === "decimal" || r === "bool")) || (r === "text" && (l === "number" || l === "decimal" || l === "bool"))) {
      return "mismatch";
    }
    return l !== "unknown" ? l : r;
  }
  return "unknown";
}

function parseExprNode(src, line, envTypes = new Map()) {
  const tokens = tokenizeDgExpr(src, line);
  let pos = 0;
  const peek = () => tokens[pos] || null;
  const peekId = () => (tokens[pos]?.type === "id" ? tokens[pos].value : null);

  function hasThenAtCurrentDepth(startPos) {
    let d = 0;
    for (let k = startPos; k < tokens.length; k++) {
      const t = tokens[k];
      if (t.type === "punct" && ["(", "[", "{"].includes(t.value)) d++;
      else if (t.type === "punct" && [")", "]", "}"].includes(t.value)) {
        if (d === 0) return false;
        d--;
      } else if (d === 0 && t.type === "id" && t.value === "then") {
        return true;
      }
    }
    return false;
  }

  function parseCond() {
    if (peekId() === "if" || (peekId() === "when" && hasThenAtCurrentDepth(pos + 1))) {
      pos++;
      const cond = parsePipe();
      if (peekId() !== "then") {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected 'then' in inline 'if/when ... then ... else ...'.",
          line,
          why: "Inline conditionals require `then` and `else`.",
          fix: 'Write `if ok then "a" else "b"` or `when ok then "a" else "b"`.',
        });
      }
      pos++;
      const yesExpr = parseCond();
      if (peekId() !== "else" && peekId() !== "otherwise") {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected 'else' in inline conditional expression.",
          line,
          why: "Inline conditionals require an `else` fallback.",
          fix: 'Write `if ok then "a" else "b"`.',
        });
      }
      pos++;
      const noExpr = parseCond();
      return { kind: "cond", cond, yesExpr, noExpr, line };
    }
    return parsePipe();
  }

  function parsePipe() {
    let left = parseNullish();
    while (peek()?.type === "op" && peek().value === "|>") {
      pos++;
      const right = parseNullish();
      if (right.kind === "call") {
        left = { kind: "call", callee: right.callee, args: [left, ...right.args], line };
      } else if (right.kind === "id") {
        left = { kind: "call", callee: right.name, args: [left], line };
      } else if (right.kind === "member") {
        const raw = right.chain.join(".");
        const callee = raw.startsWith("std.") ? right.chain[right.chain.length - 1] : raw;
        left = { kind: "call", callee, args: [left], line };
      } else {
        left = { kind: "call_expr", target: right, args: [left], line };
      }
    }
    return left;
  }

  function parseNullish() {
    let left = parseOr();
    while (peek()?.type === "op" && peek().value === "??") {
      pos++;
      left = { kind: "bin", op: "??", left, right: parseOr(), line };
    }
    return left;
  }

  function parseOr() {
    let left = parseAnd();
    while (peekId() === "or") {
      pos++;
      left = { kind: "bin", op: "||", left, right: parseAnd(), line };
    }
    return left;
  }

  function parseAnd() {
    let left = parseCmp();
    while (peekId() === "and") {
      pos++;
      left = { kind: "bin", op: "&&", left, right: parseCmp(), line };
    }
    return left;
  }

  function parseCmp() {
    let left = parseAdd();
    while (true) {
      const t = peek();
      if (!t) break;
      if (t.type === "op" && ["==", "!=", ">", "<", ">=", "<="].includes(t.value)) {
        pos++;
        const opMap = { "==": "===", "!=": "!==" };
        left = { kind: "bin", op: opMap[t.value] || t.value, left, right: parseAdd(), line };
        continue;
      }
      if (t.type === "punct" && [">", "<"].includes(t.value)) {
        pos++;
        left = { kind: "bin", op: t.value, left, right: parseAdd(), line };
        continue;
      }
      if (t.type === "id" && t.value === "equals") {
        pos++;
        left = { kind: "bin", op: "===", left, right: parseAdd(), line };
        continue;
      }
      if (t.type === "id" && t.value === "is") {
        pos++;
        let op = "===";
        if (peekId() === "not") {
          pos++;
          if (peekId() === "equal" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "to") {
            pos += 2;
          }
          op = "!==";
        } else if (peekId() === "equal" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "to") {
          pos += 2;
          op = "===";
        } else if (peekId() === "greater" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "than") {
          pos += 2;
          op = ">";
        } else if (peekId() === "less" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "than") {
          pos += 2;
          op = "<";
        } else if (peekId() === "at" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "least") {
          pos += 2;
          op = ">=";
        } else if (peekId() === "at" && tokens[pos + 1]?.type === "id" && tokens[pos + 1].value === "most") {
          pos += 2;
          op = "<=";
        } else if (peekId() === "over") {
          pos++;
          op = ">";
        } else if (peekId() === "under") {
          pos++;
          op = "<";
        }
        left = { kind: "bin", op, left, right: parseAdd(), line };
        continue;
      }
      break;
    }
    return left;
  }

  function parseAdd() {
    let left = parseMul();
    while (peek()?.type === "punct" && (peek().value === "+" || peek().value === "-")) {
      const op = tokens[pos++].value;
      const right = parseMul();
      if (op === "+") {
        const lt = inferRoughType(left, envTypes);
        const rt = inferRoughType(right, envTypes);
        if (
          (lt === "text" && (rt === "number" || rt === "decimal" || rt === "bool")) ||
          (rt === "text" && (lt === "number" || lt === "decimal" || lt === "bool"))
        ) {
          throw new DogWebDiagnostic({
            code: "E021",
            title: "Type mismatch: cannot concatenate 'text' with non-text using `+`.",
            line,
            why: "DOG forbids implicit string/number coercion.",
            fix: 'Use string interpolation `$"Value: {n}"` or explicit `text(n)`.',
          });
        }
      }
      left = { kind: "bin", op, left, right, line };
    }
    return left;
  }

  function parseMul() {
    let left = parseUnary();
    while (peek()?.type === "punct" && ["*", "/", "%"].includes(peek().value)) {
      const op = tokens[pos++].value;
      left = { kind: "bin", op, left, right: parseUnary(), line };
    }
    return left;
  }

  function parseUnary() {
    if (peekId() === "not") {
      pos++;
      return { kind: "unary", op: "!", expr: parseUnary(), line };
    }
    if (peek()?.type === "punct" && peek().value === "-") {
      pos++;
      return { kind: "unary", op: "-", expr: parseUnary(), line };
    }
    return parsePostfix();
  }

  function parsePostfix() {
    let expr = parsePrimary();
    while (peek()) {
      if (peek().type === "punct" && peek().value === "(" && (expr.kind === "id" || expr.kind === "member")) {
        pos++;
        const args = [];
        while (peek() && !(peek().type === "punct" && peek().value === ")")) {
          args.push(parseCond());
          if (peek()?.type === "punct" && peek().value === ",") pos++;
          else break;
        }
        if (peek()?.type !== "punct" || peek().value !== ")") {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Expected ')' after function arguments.",
            line,
            why: "Unclosed function call parentheses.",
            fix: "Add closing `)`.",
          });
        }
        pos++;
        let callee = expr.kind === "id" ? expr.name : expr.chain.join(".");
        // Flatten std.web.X or std.css.X or std.io.X calls cleanly
        if (callee.startsWith("std.")) {
          const parts = callee.split(".");
          callee = parts[parts.length - 1];
        }
        expr = { kind: "call", callee, args, line };
        continue;
      }
      if (peek().type === "punct" && peek().value === ".") {
        pos++;
        const prop = peek();
        if (!prop || prop.type !== "id") {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Expected property name after '.'.",
            line,
            why: "Missing field or module member identifier.",
            fix: "Write e.g. `std.web.dom_set_text(...)`.",
          });
        }
        pos++;
        if (expr.kind === "id") {
          expr = { kind: "member", chain: [expr.name, prop.value], line };
        } else if (expr.kind === "member") {
          expr = { kind: "member", chain: [...expr.chain, prop.value], line };
        } else {
          expr = { kind: "prop", target: expr, field: prop.value, line };
        }
        continue;
      }
      if (peek().type === "punct" && peek().value === "[") {
        pos++;
        let startIdx = null;
        let isSlice = false;
        let endIdx = null;
        if (peek()?.type === "op" && peek().value === "..") {
          pos++;
          isSlice = true;
          if (!(peek()?.type === "punct" && peek().value === "]")) {
            endIdx = parseOr();
          }
        } else {
          startIdx = parseOr();
          if (peek()?.type === "op" && peek().value === "..") {
            pos++;
            isSlice = true;
            if (!(peek()?.type === "punct" && peek().value === "]")) {
              endIdx = parseOr();
            }
          }
        }
        if (peek()?.type !== "punct" || peek().value !== "]") {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Expected ']' after index or slice.",
            line,
            why: "Unclosed bracket index.",
            fix: "Add closing `]`.",
          });
        }
        pos++;
        expr = isSlice
          ? { kind: "slice", target: expr, start: startIdx, end: endIdx, line }
          : { kind: "index", target: expr, index: startIdx, line };
        continue;
      }
      break;
    }
    return expr;
  }

  function parsePrimary() {
    const t = peek();
    if (!t) {
      throw new DogWebDiagnostic({
        code: "E004",
        title: "Unexpected end of expression.",
        line,
        why: "Expected a value or identifier.",
        fix: "Complete the expression.",
      });
    }
    if (t.type === "str") {
      pos++;
      return { kind: "str", value: t.value, line };
    }
    if (t.type === "interp") {
      pos++;
      const compiledParts = t.parts.map((p) =>
        p.type === "lit" ? { type: "lit", value: p.value } : { type: "expr", node: parseExprNode(p.value, line, envTypes) }
      );
      return { kind: "interp", parts: compiledParts, line };
    }
    if (t.type === "num") {
      pos++;
      return { kind: "num", value: t.value, isDecimal: t.isDecimal, line };
    }
    if (t.type === "bool") {
      pos++;
      return { kind: "bool", value: t.value, line };
    }
    if (t.type === "null") {
      pos++;
      return { kind: "null", line };
    }
    if ((t.type === "id" && (t.value === "fn" || t.value === "do")) && tokens[pos + 1]?.type === "punct" && tokens[pos + 1].value === "(") {
      pos += 2; // fn ( | do (
      const params = [];
      while (peek() && !(peek().type === "punct" && peek().value === ")")) {
        const pt = tokens[pos++];
        if (pt.type === "id") params.push(pt.value);
        if (peek()?.type === "punct" && peek().value === ",") pos++;
        else if (peek()?.type === "punct" && peek().value === ":") {
          pos++;
          if (peek()) pos++;
          if (peek()?.type === "punct" && peek().value === ",") pos++;
        }
      }
      if (peek()?.type === "punct" && peek().value === ")") pos++;
      if (!(peek()?.type === "op" && peek().value === "=>")) {
        throw new DogWebDiagnostic({ code: "E004", title: "Expected '=>' in lambda expression.", line, why: "Inline lambda requires `=>`.", fix: "Write `fn(x) => x * 2` or `do(x) => x * 2`." });
      }
      pos++;
      const body = parseCond();
      return { kind: "lambda", params, body, line };
    }
    if (t.type === "id") {
      pos++;
      return { kind: "id", name: t.value, line };
    }
    if (t.type === "punct" && t.value === "(") {
      pos++;
      const inner = parseCond();
      if (peek()?.type !== "punct" || peek().value !== ")") {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected ')'.",
          line,
          why: "Unclosed parenthesis.",
          fix: "Add closing `)`.",
        });
      }
      pos++;
      return { kind: "group", inner, line };
    }
    if (t.type === "punct" && t.value === "[") {
      pos++;
      const elements = [];
      while (peek() && !(peek().type === "punct" && peek().value === "]")) {
        elements.push(parseCond());
        if (peek()?.type === "punct" && peek().value === ",") pos++;
        else break;
      }
      if (peek()?.type !== "punct" || peek().value !== "]") {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected ']' closing list literal.",
          line,
          why: "List was not closed with ']'.",
          fix: "Add `]`.",
        });
      }
      pos++;
      return { kind: "list", elements, line };
    }
    if (t.type === "punct" && t.value === "{") {
      pos++;
      const entries = [];
      while (peek() && !(peek().type === "punct" && peek().value === "}")) {
        const k = parseCond();
        if (peek()?.type !== "punct" || peek().value !== ":") {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Expected ':' in map literal.",
            line,
            why: "Map entries require `key: value`.",
            fix: 'Example: `{"Home": "#top"}`',
          });
        }
        pos++;
        const v = parseCond();
        entries.push([k, v]);
        if (peek()?.type === "punct" && peek().value === ",") pos++;
        else break;
      }
      if (peek()?.type !== "punct" || peek().value !== "}") {
        throw new DogWebDiagnostic({
          code: "E004",
          title: "Expected '}' closing map literal.",
          line,
          why: "Map literal was not closed with '}'.",
          fix: "Add `}`.",
        });
      }
      pos++;
      return { kind: "map", entries, line };
    }
    throw new DogWebDiagnostic({
      code: "E004",
      title: `Unexpected token '${t.value || t.type}' in expression.`,
      line,
      why: "Invalid expression syntax.",
      fix: "Check operators and punctuation.",
    });
  }

  return parseCond();
}

function emitJsFromNode(node) {
  switch (node.kind) {
    case "str":
      return JSON.stringify(node.value);
    case "interp": {
      if (node.parts.length === 0) return '""';
      return (
        "(" +
        node.parts
          .map((p) => (p.type === "lit" ? JSON.stringify(p.value) : `String(${emitJsFromNode(p.node)})`))
          .join(" + ") +
        ")"
      );
    }
    case "num":
      return String(node.value);
    case "bool":
      return node.value ? "true" : "false";
    case "null":
      return "null";
    case "id":
      return node.name;
    case "group":
      return `(${emitJsFromNode(node.inner)})`;
    case "member":
      return node.chain.join(".");
    case "prop":
      return `(${emitJsFromNode(node.target)}).${node.field}`;
    case "index":
      return `(${emitJsFromNode(node.target)})[${emitJsFromNode(node.index)}]`;
    case "slice":
      return `(${emitJsFromNode(node.target)}).slice(${node.start ? emitJsFromNode(node.start) : 0}${node.end ? `, ${emitJsFromNode(node.end)}` : ""})`;
    case "lambda":
      return `((${node.params.join(", ")}) => (${emitJsFromNode(node.body)}))`;
    case "call_expr":
      return `(${emitJsFromNode(node.target)})(${node.args.map(emitJsFromNode).join(", ")})`;
    case "list":
      return `[${node.elements.map(emitJsFromNode).join(", ")}]`;
    case "map":
      return `({ ${node.entries.map(([k, v]) => `[${emitJsFromNode(k)}]: ${emitJsFromNode(v)}`).join(", ")} })`;
    case "unary":
      return `(${node.op}(${emitJsFromNode(node.expr)}))`;
    case "bin":
      return `(${emitJsFromNode(node.left)} ${node.op} ${emitJsFromNode(node.right)})`;
    case "cond":
      return `((${emitJsFromNode(node.cond)}) ? (${emitJsFromNode(node.yesExpr)}) : (${emitJsFromNode(node.noExpr)}))`;
    case "call": {
      const a = node.args.map(emitJsFromNode);
      const mapCall = {
        show: () => `__dogWeb.print(${a[0]})`,
        say: () => `__dogWeb.print(${a[0]})`,
        print: () => `__dogWeb.print(${a[0]})`,
        check: () => `(() => { if (!(${a[0]})) { throw new Error(${a[1] || '"Check failed"'}); } return true; })()`,
        assert: () => `(() => { if (!(${a[0]})) { throw new Error(${a[1] || '"Assertion failed"'}); } return true; })()`,
        assert_eq: () => `(() => { if (JSON.stringify(${a[0]}) !== JSON.stringify(${a[1]})) { throw new Error("Assertion failed: expected equal values"); } return true; })()`,
        cli_info: () => `__dogWeb.print(${a[0]})`,
        cli_success: () => `__dogWeb.print(${a[0]})`,
        cli_warn: () => `__dogWeb.print(${a[0]})`,
        cli_error: () => `__dogWeb.print(${a[0]})`,
        text: () => `String(${a[0]})`,
        number: () => `parseInt(String(${a[0]}), 10)`,
        decimal: () => `Number(${a[0]})`,
        abs: () => `Math.abs(${a[0]})`,
        min: () => `Math.min(${a[0]}, ${a[1]})`,
        max: () => `Math.max(${a[0]}, ${a[1]})`,
        round: () => `Math.round(${a[0]})`,
        floor: () => `Math.floor(${a[0]})`,
        ceil: () => `Math.ceil(${a[0]})`,
        clamp: () => `Math.min(Math.max(${a[0]}, ${a[1]}), ${a[2]})`,
        lower: () => `String(${a[0]}).toLowerCase()`,
        upper: () => `String(${a[0]}).toUpperCase()`,
        trim: () => `String(${a[0]}).trim()`,
        starts_with: () => `String(${a[0]}).startsWith(String(${a[1]}))`,
        ends_with: () => `String(${a[0]}).endsWith(String(${a[1]}))`,
        contains: () => `String(${a[0]}).includes(String(${a[1]}))`,
        split: () => `String(${a[0]}).split(String(${a[1]}))`,
        join: () => `(${a[0]}).join(String(${a[1]}))`,
        len: () => `(Array.isArray(${a[0]}) || typeof ${a[0]} === "string" ? ${a[0]}.length : Object.keys(${a[0]} || {}).length)`,
        push: () => `(${a[0]}).push(${a[1]})`,
        pop: () => `(${a[0]}).pop()`,
        clear: () => (a.length === 0 ? "console.clear()" : `(${a[0]}.length = 0)`),
        range: () => `Array.from({ length: Math.max(0, (${a[1]}) - (${a[0]})) }, (_, i) => (${a[0]}) + i)`,
        map_get: () => `(${a[0]})[${a[1]}]`,
        map_opt: () => `(Object.prototype.hasOwnProperty.call(${a[0]}, ${a[1]}) ? (${a[0]})[${a[1]}] : null)`,
        map_get_or: () => `(Object.prototype.hasOwnProperty.call(${a[0]}, ${a[1]}) ? (${a[0]})[${a[1]}] : ${a[2]})`,
        map_set: () => `((${a[0]})[${a[1]}] = ${a[2]})`,
        map_has: () => `Object.prototype.hasOwnProperty.call(${a[0]}, ${a[1]})`,
        map_keys: () => `Object.keys(${a[0]})`,
        map_values: () => `Object.values(${a[0]})`,
        json_stringify: () => `JSON.stringify(${a[0]})`,
        json_parse: () => `JSON.parse(${a[0]})`,
        html_escape: () => `__dogEscape(${a[0]})`,
        dog_css: () => `__dogWeb.dogCss(${a[0]})`,
        compile: () => `__dogWeb.dogCss(${a[0]})`,
        css_rule: () => `__dogWeb.cssRule(${a[0]}, ${a[1]})`,
        css_vars: () => `__dogWeb.cssVars(${a[0]})`,
        css_media: () => `__dogWeb.cssMedia(${a[0]}, ${a[1]})`,
        css_keyframes: () => `__dogWeb.cssKeyframes(${a[0]}, ${a[1]})`,
        css_theme: () => `__dogWeb.cssTheme(${a[0]}, ${a[1]}, ${a[2]}, ${a[3]}, ${a[4]})`,
        css_bundle: () => (a.length === 0 ? "__dogWeb.cssBundle()" : `(${a[0]}).join("\\n\\n")`),
        html_el: () => `__dogWeb.htmlEl(${a[0]}, ${a[1]}, ${a[2]})`,
        html_page: () => `__dogWeb.htmlPage(${a[0]}, ${a[1]}, ${a[2]})`,
        web_nav: () => `__dogWeb.webNav(${a[0]}, ${a[1]})`,
        nav: () => `__dogWeb.webNav(${a[0]}, ${a[1]})`,
        web_card: () => `__dogWeb.webCard(${a[0]}, ${a[1]})`,
        card: () => `__dogWeb.webCard(${a[0]}, ${a[1]})`,
        web_grid: () => `__dogWeb.webGrid(${a[0]}, ${a[1]})`,
        grid: () => `__dogWeb.webGrid(${a[0]}, ${a[1]})`,
        web_button: () => `__dogWeb.webButton(${a[0]}, ${a[1]})`,
        button: () => `__dogWeb.webButton(${a[0]}, ${a[1]})`,
        web_input: () => `__dogWeb.webInput(${a[0]}, ${a[1]}, ${a[2]})`,
        input: () => `__dogWeb.webInput(${a[0]}, ${a[1]}, ${a[2]})`,
        web_badge: () => `__dogWeb.webBadge(${a[0]}, ${a[1] || '""'})`,
        web_alert: () => `__dogWeb.webAlert(${a[0]}, ${a[1]}, ${a[2] || '"info"'})`,
        web_field: () => `__dogWeb.webField(${a[0]}, ${a[1]}, ${a[2] || '""'}, ${a[3] || '"text"'})`,
        web_form: () => `__dogWeb.webForm(${a[0]}, ${a[1]}, ${a[2] || '"Submit"'})`,
        web_table: () => `__dogWeb.webTable(${a[0]}, ${a[1]})`,
        web_progress: () => `__dogWeb.webProgress(${a[0]}, ${a[1]}, ${a[2] || "100"})`,
        web_toast: () => `__dogWeb.webToast(${a[0]}, ${a[1] || '"info"'})`,
        web_validate_required: () => `__dogWeb.webValidateRequired(${a[0]}, ${a[1] || '""'})`,
        web_validate_email: () => `__dogWeb.webValidateEmail(${a[0]})`,
        dom_focus: () => `__dogWeb.domFocus(${a[0]})`,
        dom_attr: () => `__dogWeb.domAttr(${a[0]}, ${a[1]})`,
        dom_set_attr: () => `__dogWeb.domSetAttr(${a[0]}, ${a[1]}, ${a[2]})`,
        dom_mount: () => `__dogWeb.domMount(${a[0]}, ${a[1]})`,
        mount: () => `__dogWeb.domMount(${a[0]}, ${a[1]})`,
        dom_set_text: () => `__dogWeb.domSetText(${a[0]}, ${a[1]})`,
        set_text: () => `__dogWeb.domSetText(${a[0]}, ${a[1]})`,
        dom_set_html: () => `__dogWeb.domSetHtml(${a[0]}, ${a[1]})`,
        set_html: () => `__dogWeb.domSetHtml(${a[0]}, ${a[1]})`,
        dom_get_value: () => `__dogWeb.domGetValue(${a[0]})`,
        get_value: () => `__dogWeb.domGetValue(${a[0]})`,
        dom_set_value: () => `__dogWeb.domSetValue(${a[0]}, ${a[1]})`,
        set_value: () => `__dogWeb.domSetValue(${a[0]}, ${a[1]})`,
        dom_add_class: () => `__dogWeb.domAddClass(${a[0]}, ${a[1]})`,
        add_class: () => `__dogWeb.domAddClass(${a[0]}, ${a[1]})`,
        dom_remove_class: () => `__dogWeb.domRemoveClass(${a[0]}, ${a[1]})`,
        remove_class: () => `__dogWeb.domRemoveClass(${a[0]}, ${a[1]})`,
        dom_toggle_class: () => `__dogWeb.domToggleClass(${a[0]}, ${a[1]})`,
        toggle_class: () => `__dogWeb.domToggleClass(${a[0]}, ${a[1]})`,
        dom_set_style: () => `__dogWeb.domSetStyle(${a[0]}, ${a[1]}, ${a[2]})`,
        set_style: () => `__dogWeb.domSetStyle(${a[0]}, ${a[1]}, ${a[2]})`,
        dom_on: () => `__dogWeb.domOn(${a[0]}, ${a[1]}, ${a[2]})`,
        on: () => `__dogWeb.domOn(${a[0]}, ${a[1]}, ${a[2]})`,
        web_title: () => `__dogWeb.webTitle(${a[0]})`,
        web_component: () => `__dogWeb.webComponent(${a[0]}, ${a[1]})`,
        web_render_component: () => `__dogWeb.webRenderComponent(${a[0]}, ${a[1] || "{}"})`,
        web_state_set: () => `__dogWeb.webStateSet(${a[0]}, ${a[1]})`,
        web_state_get: () => `__dogWeb.webStateGet(${a[0]}, ${a[1] || '""'})`,
        web_on_state: () => `__dogWeb.webOnState(${a[0]}, ${a[1]})`,
        dom_delegate: () => `__dogWeb.domDelegate(${a[0]}, ${a[1]}, ${a[2]}, ${a[3]})`,
        web_tabs: () => `__dogWeb.webTabs(${a[0]}, ${a[1]}, ${a[2] || '""'})`,
        web_modal: () => `__dogWeb.webModal(${a[0]}, ${a[1]}, ${a[2]})`,
        web_storage_set: () => `__dogWeb.webStorageSet(${a[0]}, ${a[1]})`,
        storage_set: () => `__dogWeb.webStorageSet(${a[0]}, ${a[1]})`,
        web_storage_get: () => `__dogWeb.webStorageGet(${a[0]})`,
        storage_get: () => `__dogWeb.webStorageGet(${a[0]})`,
        web_route: () => "__dogWeb.webRoute()",
        route: () => "__dogWeb.webRoute()",
        web_navigate: () => `__dogWeb.webNavigate(${a[0]})`,
        navigate: () => `__dogWeb.webNavigate(${a[0]})`,
      };
      if (mapCall[node.callee]) return mapCall[node.callee]();
      return `${node.callee}(${a.join(", ")})`;
    }
    default:
      return "undefined";
  }
}

export function compileDogSourceToJs(source) {
  const rawLines = String(source || "").split(/\r?\n/);
  const envTypes = new Map();
  const lines = [];
  let bufText = "";
  let bufLine = 1;
  let depth = 0;

  function deltaBracketDepth(str) {
    let d = 0;
    let inStr = false;
    let esc = false;
    for (let i = 0; i < str.length; i++) {
      const ch = str[i];
      if (esc) { esc = false; continue; }
      if (inStr) {
        if (ch === "\\") esc = true;
        else if (ch === '"') inStr = false;
        continue;
      }
      if (ch === '"') { inStr = true; continue; }
      if (ch === "(" || ch === "[" || ch === "{") d++;
      else if (ch === ")" || ch === "]" || ch === "}") d--;
    }
    return d;
  }

  rawLines.forEach((raw, idx) => {
    const t = stripDgComment(raw).trim();
    if (!t) return;
    if (depth > 0) {
      bufText += ` ${t}`;
      depth += deltaBracketDepth(t);
      if (depth <= 0) {
        lines.push({ line: bufLine, text: bufText });
        bufText = "";
        depth = 0;
      }
    } else {
      const d = deltaBracketDepth(t);
      if (d > 0) {
        bufText = t;
        bufLine = idx + 1;
        depth = d;
      } else {
        lines.push({ line: idx + 1, text: t });
      }
    }
  });
  if (bufText) lines.push({ line: bufLine, text: bufText });

  let cursor = 0;

  function splitTopLevelRange(expr) {
    let depth = 0;
    let inStr = false;
    let esc = false;
    for (let i = 0; i < expr.length - 1; i++) {
      const ch = expr[i];
      if (esc) { esc = false; continue; }
      if (inStr) {
        if (ch === "\\") esc = true;
        else if (ch === '"') inStr = false;
        continue;
      }
      if (ch === '"') { inStr = true; continue; }
      if ("([{".includes(ch)) depth++;
      else if (")]}".includes(ch)) depth = Math.max(0, depth - 1);
      else if (depth === 0 && ch === "." && expr[i + 1] === ".") {
        return [expr.slice(0, i).trim(), expr.slice(i + 2).trim()];
      }
    }
    return null;
  }

  function parseBlock(stopTokens) {
    const out = [];
    while (cursor < lines.length) {
      const { text } = lines[cursor];
      if (stopTokens.some((tok) => text === tok || text.startsWith(`${tok} `))) {
        return out;
      }
      out.push(parseStmt());
    }
    if (stopTokens.length > 0) {
      throw new DogWebDiagnostic({
        code: "E005",
        title: "Missing `end` closing block.",
        line: lines[lines.length - 1]?.line || 1,
        why: `Reached end of file while expecting ${stopTokens.join(" or ")}.`,
        fix: "Add closing `end` for each open block (`define`, `do`, `if`, `when`, `while`, `for`, `each`, `repeat`, `loop`, `try`).",
      });
    }
    return out;
  }

  function parseStmt() {
    const { line, text } = lines[cursor++];

    if (text.startsWith("use ")) {
      return `// ${text}`;
    }
    if (/^type\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=/.test(text)) {
      return `// ${text}`;
    }
    if (text === "break" || text === "stop") {
      return "break;";
    }
    if (text === "continue" || text === "skip") {
      return "continue;";
    }
    if (text === "return" || text === "give") {
      return "return;";
    }
    if (text.startsWith("return ")) {
      const node = parseExprNode(text.slice(7).trim(), line, envTypes);
      return `return ${emitJsFromNode(node)};`;
    }
    if (text.startsWith("give ") && !/^give\s*=/.test(text)) {
      const node = parseExprNode(text.slice(5).trim(), line, envTypes);
      return `return ${emitJsFromNode(node)};`;
    }

    // Inline DOG CSS block: style theme ... end | style ".card" ... end
    if (text === "style" || text.startsWith("style ")) {
      const targetRaw = text.slice(5).trim();
      if (!targetRaw) {
        throw new DogWebDiagnostic({
          code: "E004",
          title: 'Expected selector or `theme` after `style`.',
          line,
          why: "A `style` block needs a CSS selector string or `theme` keyword.",
          fix: 'Write e.g. `style ".card"` or `style theme`.',
        });
      }
      const isTheme = targetRaw === "theme" || targetRaw === "root" || targetRaw === "vars";
      const selJs = isTheme
        ? '":root"'
        : emitJsFromNode(parseExprNode(targetRaw, line, envTypes));

      const baseProps = [];
      const variantBlocks = [];

      function parseStyleVariantTrigger(vText) {
        const clean = vText.replace(/^on\s+/, "").trim();
        const direct = {
          hover: ":hover",
          focus: ":focus",
          active: ":active",
          mobile: "@media (max-width: 640px)",
          tablet: "@media (max-width: 960px)",
          desktop: "@media (min-width: 1024px)",
          dark: "@media (prefers-color-scheme: dark)",
        };
        if (!vText.includes(":") && direct[clean]) return direct[clean];
        const m = vText.match(/^media\s+"([^"]+)"$/);
        if (m) return `@media ${m[1]}`;
        return null;
      }

      function parseStylePropLine(sText, sLine) {
        const cleaned = sText.replace(/,\s*$/, "").trim();
        const colonIdx = cleaned.indexOf(":");
        if (colonIdx <= 0) {
          throw new DogWebDiagnostic({
            code: "E004",
            title: `Expected \`property: value\` inside \`style\` block, found \`${sText}\`.`,
            line: sLine,
            why: "Style declarations use `prop: expr` (e.g. `bg: \"#FFFFFF\"` or `pad: \"16px\"`).",
            fix: 'Write e.g. `bg: "#FFFFFF"` or close the block with `end`.',
          });
        }
        const prop = cleaned.slice(0, colonIdx).trim();
        const valRaw = cleaned.slice(colonIdx + 1).trim();
        const valNode = parseExprNode(valRaw, sLine, envTypes);
        return { prop, valJs: emitJsFromNode(valNode) };
      }

      while (cursor < lines.length && lines[cursor].text !== "end") {
        const inner = lines[cursor++];
        const trig = parseStyleVariantTrigger(inner.text);
        if (trig) {
          const vRules = [];
          while (cursor < lines.length && lines[cursor].text !== "end") {
            const vLine = lines[cursor++];
            vRules.push(parseStylePropLine(vLine.text, vLine.line));
          }
          if (lines[cursor]?.text !== "end") {
            throw new DogWebDiagnostic({
              code: "E005",
              title: "Missing `end` for style variant block.",
              line: inner.line,
              why: "Nested `on hover` / `on mobile` blocks inside `style` must close with `end`.",
              fix: "Add `end` on its own line.",
            });
          }
          cursor++;
          variantBlocks.push({ trig, rules: vRules });
        } else {
          baseProps.push(parseStylePropLine(inner.text, inner.line));
        }
      }

      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({
          code: "E005",
          title: "Missing `end` closing `style` block.",
          line,
          why: 'Every `style "<selector>" ... end` block must end with `end`.',
          fix: "Add `end` after your style rules.",
        });
      }
      cursor++;

      const emitRuleCall = (mediaExpr, sJs, propList) => {
        if (propList.length === 0) return "";
        const obj = `{ ${propList.map((p) => `${JSON.stringify(p.prop)}: __dogWeb.toText(${p.valJs})`).join(", ")} }`;
        if (mediaExpr) {
          return `__dogWeb.styleMedia(${JSON.stringify(mediaExpr)}, ${sJs}, ${obj});`;
        }
        return `__dogWeb.styleRule(${sJs}, ${obj});`;
      };

      const calls = [];
      if (baseProps.length > 0) {
        calls.push(emitRuleCall(null, selJs, baseProps));
      }
      for (const vb of variantBlocks) {
        if (vb.trig.startsWith(":")) {
          calls.push(emitRuleCall(null, `(${selJs}) + ${JSON.stringify(vb.trig)}`, vb.rules));
        } else {
          const q = vb.trig.replace(/^@media\s*/, "").trim();
          calls.push(emitRuleCall(q, selJs, vb.rules));
        }
      }
      return calls.join("\n");
    }

    // Destructuring: let / var / const / keep / make [a, b] = list
    const destructMatch = text.match(/^(let|var|const|keep|make)\s+\[([^\]]+)\]\s*=\s*(.+)$/);
    if (destructMatch) {
      const [, kw, rawNames, exprStr] = destructMatch;
      const node = parseExprNode(exprStr, line, envTypes);
      const names = rawNames.split(",").map((s) => s.trim()).filter(Boolean);
      const jsKw = (kw === "var" || kw === "make") ? "let" : "const";
      return `${jsKw} [${names.join(", ")}] = ${emitJsFromNode(node)};`;
    }

    // Bindings: let / var / const / keep / make
    const bindMatch = text.match(/^(let|var|const|keep|make)\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*:\s*[^=]+)?\s*=\s*(.+)$/);
    if (bindMatch) {
      const [, kw, name, exprStr] = bindMatch;
      const node = parseExprNode(exprStr, line, envTypes);
      envTypes.set(name, inferRoughType(node, envTypes));
      const jsKw = (kw === "var" || kw === "make") ? "let" : "const";
      return `${jsKw} ${name} = ${emitJsFromNode(node)};`;
    }

    // Natural English & Simple Style assignments
    const setMatch = text.match(/^set\s+(.+?)\s+to\s+(.+)$/);
    if (setMatch) {
      const [, lhs, rhs] = setMatch;
      const node = parseExprNode(rhs, line, envTypes);
      return `${lhs.trim()} = ${emitJsFromNode(node)};`;
    }
    const incMatch = text.match(/^(increase|decrease)\s+(.+?)\s+by\s+(.+)$/);
    if (incMatch) {
      const [, verb, lhs, rhs] = incMatch;
      const node = parseExprNode(rhs, line, envTypes);
      const op = verb === "increase" ? "+=" : "-=";
      return `${lhs.trim()} ${op} ${emitJsFromNode(node)};`;
    }
    const addToMatch = text.match(/^add\s+(.+?)\s+to\s+([a-zA-Z_][a-zA-Z0-9_.\[\]]*)$/);
    if (addToMatch) {
      const node = parseExprNode(addToMatch[1], line, envTypes);
      return `${addToMatch[2].trim()} += ${emitJsFromNode(node)};`;
    }
    const takeFromMatch = text.match(/^take\s+(.+?)\s+from\s+([a-zA-Z_][a-zA-Z0-9_.\[\]]*)$/);
    if (takeFromMatch) {
      const node = parseExprNode(takeFromMatch[1], line, envTypes);
      return `${takeFromMatch[2].trim()} -= ${emitJsFromNode(node)};`;
    }
    const bumpMatch = text.match(/^bump\s+([a-zA-Z_][a-zA-Z0-9_.\[\]]*)(?:\s+by\s+(.+))?$/);
    if (bumpMatch) {
      const deltaJs = bumpMatch[2] ? emitJsFromNode(parseExprNode(bumpMatch[2], line, envTypes)) : "1";
      return `${bumpMatch[1].trim()} += ${deltaJs};`;
    }
    const showStmtMatch = text.match(/^(?:show|print|say)\s+(?![=(])(.+)$/);
    if (showStmtMatch) {
      const node = parseExprNode(showStmtMatch[1].trim(), line, envTypes);
      return `__dogWeb.print(${emitJsFromNode(node)});`;
    }
    const checkStmtMatch = text.match(/^(?:check|assert)\s+(?![=(])(.+)$/);
    if (checkStmtMatch) {
      const node = parseExprNode(checkStmtMatch[1].trim(), line, envTypes);
      return `if (!(${emitJsFromNode(node)})) { throw new Error("Check failed"); }`;
    }

    // Compound assignment & direct assignment
    const compMatch = text.match(/^([a-zA-Z_][a-zA-Z0-9_.\[\]]*)\s*(\+=|-=|\*=|\/=|%=|=)\s*(.+)$/);
    if (compMatch) {
      const [, lhs, op, rhs] = compMatch;
      const node = parseExprNode(rhs, line, envTypes);
      return `${lhs} ${op} ${emitJsFromNode(node)};`;
    }

    // One-liner function across all styles: define|fn|function|do name(params) [-> T | returns T] = expr
    const simpleOneLinerMatch = text.match(/^(?:define|fn|function|do)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)(?:\s*(?:->|returns)\s*[^=]+)?\s*=\s*(.+)$/);
    if (simpleOneLinerMatch) {
      const [, name, rawParams, bodyExpr] = simpleOneLinerMatch;
      const params = splitTopLevelCommas(rawParams).map((p) => {
        const eqIdx = p.indexOf("=");
        let defJs = "";
        let left = p;
        if (eqIdx >= 0) {
          left = p.slice(0, eqIdx).trim();
          const defNode = parseExprNode(p.slice(eqIdx + 1).trim(), line, envTypes);
          defJs = ` = ${emitJsFromNode(defNode)}`;
        }
        const colonIdx = left.indexOf(":");
        const pname = (colonIdx >= 0 ? left.slice(0, colonIdx) : left).trim();
        envTypes.set(pname, "unknown");
        return `${pname}${defJs}`;
      });
      const retNode = parseExprNode(bodyExpr.trim(), line, envTypes);
      return `function ${name}(${params.join(", ")}) {\n  return ${emitJsFromNode(retNode)};\n}\n__dogFns[${JSON.stringify(name)}] = ${name};`;
    }

    // Functions: define / fn / function / do
    const fnMatch = text.match(/^(?:define|fn|function|do)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)(?:\s*(?:returns|->)\s*(.+))?$/);
    if (fnMatch) {
      const [, name, rawParams] = fnMatch;
      const params = splitTopLevelCommas(rawParams).map((p) => {
        const eqIdx = p.indexOf("=");
        let defJs = "";
        let left = p;
        if (eqIdx >= 0) {
          left = p.slice(0, eqIdx).trim();
          const defNode = parseExprNode(p.slice(eqIdx + 1).trim(), line, envTypes);
          defJs = ` = ${emitJsFromNode(defNode)}`;
        }
        const colonIdx = left.indexOf(":");
        const pname = (colonIdx >= 0 ? left.slice(0, colonIdx) : left).trim();
        const ptype = colonIdx >= 0 ? left.slice(colonIdx + 1).trim() : "unknown";
        envTypes.set(pname, ptype);
        return `${pname}${defJs}`;
      });
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({
          code: "E005",
          title: `Missing \`end\` for function '${name}'.`,
          line,
          why: "Function block was not closed.",
          fix: "Add `end` at the end of the function.",
        });
      }
      cursor++;
      return `function ${name}(${params.join(", ")}) {\n${body.map((s) => `  ${s}`).join("\n")}\n}\n__dogFns[${JSON.stringify(name)}] = ${name};`;
    }

    // if / unless / when
    if (text.startsWith("if ") || text.startsWith("unless ") || text.startsWith("when ")) {
      const isUnless = text.startsWith("unless ");
      const leadLen = isUnless ? 7 : text.startsWith("when ") ? 5 : 3;
      const rawCond = text.slice(leadLen).replace(/\s+then$/, "").trim();
      const condNode = parseExprNode(rawCond, line, envTypes);
      const condJs = isUnless ? `!(${emitJsFromNode(condNode)})` : emitJsFromNode(condNode);
      const branches = [{ condJs, body: parseBlock(["elif", "or when", "else", "otherwise", "end"]) }];

      while (cursor < lines.length && lines[cursor].text !== "end") {
        const kwLine = lines[cursor++];
        const kt = kwLine.text;
        if (kt.startsWith("elif ")) {
          const c = kt.slice(5).replace(/\bthen\s*$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "or when", "else", "otherwise", "end"]),
          });
        } else if (kt.startsWith("or when ")) {
          const c = kt.slice(8).replace(/\bthen\s*$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "or when", "else", "otherwise", "end"]),
          });
        } else if (kt.startsWith("else if ")) {
          const c = kt.slice(8).replace(/\bthen\s*$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "or when", "else", "otherwise", "end"]),
          });
        } else if (kt.startsWith("otherwise if ")) {
          const c = kt.slice(13).replace(/\bthen\s*$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "or when", "else", "otherwise", "end"]),
          });
        } else if (kt === "else" || kt === "otherwise") {
          branches.push({ condJs: null, body: parseBlock(["end"]) });
          break;
        }
      }
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({
          code: "E005",
          title: "Missing `end` after conditional block.",
          line,
          why: "Open `if` / `unless` / `when` block has no closing `end`.",
          fix: "Add `end`.",
        });
      }
      cursor++;
      return branches
        .map((b, i) => {
          const head = i === 0 ? `if (${b.condJs})` : b.condJs ? `else if (${b.condJs})` : "else";
          return `${head} {\n${b.body.map((s) => `  ${s}`).join("\n")}\n}`;
        })
        .join(" ");
    }

    // while / until
    if (text.startsWith("while ") || text.startsWith("until ")) {
      const isUntil = text.startsWith("until ");
      const rawCond = text.slice(6).replace(/\s+then$/, "").trim();
      const condJs = emitJsFromNode(parseExprNode(rawCond, line, envTypes));
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after loop.", line, why: "Loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `while (${isUntil ? `!(${condJs})` : condJs}) {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // repeat N [times] / loop N [times] / loop
    if (text.startsWith("repeat ") || text === "loop" || text.startsWith("loop ")) {
      const isInfinite = text === "loop";
      const rawCount = isInfinite ? "" : (text.startsWith("loop ") ? text.slice(5) : text.slice(7)).replace(/\btimes\s*$/, "").trim();
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after loop block.", line, why: "Loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      if (isInfinite) {
        return `while (true) {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
      }
      const countJs = emitJsFromNode(parseExprNode(rawCount, line, envTypes));
      return `for (let __rep = 0; __rep < (${countJs}); __rep++) {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // each x in a..b | each x in list | each x from A to B
    if (text.startsWith("each ")) {
      const eachFromMatch = text.match(/^each\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
      let headJs = "";
      if (eachFromMatch) {
        const [, varName, sExpr, eExpr] = eachFromMatch;
        const sJs = emitJsFromNode(parseExprNode(sExpr, line, envTypes));
        const eJs = emitJsFromNode(parseExprNode(eExpr, line, envTypes));
        headJs = `for (let ${varName} = (${sJs}); ${varName} <= (${eJs}); ${varName}++)`;
      } else {
        const eachMatch = text.match(/^each\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+(.+)$/);
        if (!eachMatch) {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Invalid `each` loop header.",
            line,
            why: `Could not parse \`${text}\`.`,
            fix: "Use `each i in 1..5`, `each i from 1 to 5`, or `each item in list`.",
          });
        }
        const [, varName, iterRaw] = eachMatch;
        const rangeParts = splitTopLevelRange(iterRaw);
        if (rangeParts) {
          const sJs = emitJsFromNode(parseExprNode(rangeParts[0], line, envTypes));
          const eJs = emitJsFromNode(parseExprNode(rangeParts[1], line, envTypes));
          headJs = `for (let ${varName} = (${sJs}); ${varName} <= (${eJs}); ${varName}++)`;
        } else {
          headJs = `for (const ${varName} of ${emitJsFromNode(parseExprNode(iterRaw, line, envTypes))})`;
        }
      }
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after `each` loop.", line, why: "Each loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `${headJs} {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // for i from A to B | for each x in list | for i in A..B
    if (text.startsWith("for ")) {
      const fromMatch = text.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
      let headJs = "";
      if (fromMatch) {
        const [, varName, sExpr, eExpr] = fromMatch;
        const sJs = emitJsFromNode(parseExprNode(sExpr, line, envTypes));
        const eJs = emitJsFromNode(parseExprNode(eExpr, line, envTypes));
        headJs = `for (let ${varName} = (${sJs}); ${varName} <= (${eJs}); ${varName}++)`;
      } else {
        const eachMatch = text.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+(.+)$/);
        if (!eachMatch) {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Invalid `for` loop header.",
            line,
            why: `Could not parse \`${text}\`.`,
            fix: "Use `for i from 1 to 5`, `for i in 1..5`, or `for each item in list`.",
          });
        }
        const [, varName, listExpr] = eachMatch;
        const rangeParts = splitTopLevelRange(listExpr);
        if (rangeParts) {
          const sJs = emitJsFromNode(parseExprNode(rangeParts[0], line, envTypes));
          const eJs = emitJsFromNode(parseExprNode(rangeParts[1], line, envTypes));
          headJs = `for (let ${varName} = (${sJs}); ${varName} <= (${eJs}); ${varName}++)`;
        } else {
          headJs = `for (const ${varName} of ${emitJsFromNode(parseExprNode(listExpr, line, envTypes))})`;
        }
      }
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after `for` loop.", line, why: "For loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `${headJs} {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // try ... catch err / oops err ... end
    if (text === "try") {
      const tryBody = parseBlock(["catch", "oops", "end"]);
      let catchVar = "err";
      let catchBody = [];
      if (lines[cursor]?.text.startsWith("catch") || lines[cursor]?.text.startsWith("oops")) {
        catchVar = lines[cursor].text.replace(/^(catch|oops)\s*/, "").trim() || "err";
        cursor++;
        catchBody = parseBlock(["end"]);
      }
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after `try/catch`.", line, why: "Try block is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `try {\n${tryBody.map((s) => `  ${s}`).join("\n")}\n} catch (__e) {\n  const ${catchVar} = (__e && __e.message) ? __e.message : String(__e);\n${catchBody.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // Expression statement
    const exprNode = parseExprNode(text, line, envTypes);
    return `${emitJsFromNode(exprNode)};`;
  }

  const stmts = parseBlock([]);
  return stmts.join("\n");
}

const RUNTIME_HEADER = `"use strict";
const __dogFns = Object.create(null);
const __dogStorageFallback = Object.create(null);
const __dogCssVars = [];
const __dogStyleRules = [];
function __dogQ(sel) {
  if (typeof document === "undefined") return null;
  return document.querySelector(sel) || document.getElementById(String(sel).replace(/^#/, ""));
}
function __dogEscape(s) {
  return String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;").replace(/'/g,"&#39;");
}
function __dogExpandVarRefs(raw) {
  return String(raw).replace(/\\$([a-zA-Z_][a-zA-Z0-9_-]*)/g, "var(--$1)");
}
function __dogExpandDecl(rawKey, rawVal) {
  const prop = String(rawKey).trim().toLowerCase();
  const val = __dogExpandVarRefs(String(rawVal).trim());
  switch (prop) {
    case "bg": return [["background", val]];
    case "ink": case "fg": case "text": return [["color", val]];
    case "pad": case "p": return [["padding", val]];
    case "px": case "pad-x": return [["padding-inline", val]];
    case "py": case "pad-y": return [["padding-block", val]];
    case "m": return [["margin", val]];
    case "mx": case "margin-x": return [["margin-inline", val]];
    case "my": case "margin-y": return [["margin-block", val]];
    case "w": return [["width", val]];
    case "h": return [["height", val]];
    case "max-w": return [["max-width", val]];
    case "min-w": return [["min-width", val]];
    case "max-h": return [["max-height", val]];
    case "min-h": return [["min-height", val]];
    case "radius": case "round": return [["border-radius", val]];
    case "size": return [["font-size", val]];
    case "weight": return [["font-weight", val]];
    case "font": return [["font-family", val]];
    case "leading": return [["line-height", val]];
    case "ease": return [["transition", val]];
    case "align": return [["align-items", val]];
    case "justify": return [["justify-content", val]];
    case "cols": return [["display", "grid"], ["grid-template-columns", /^\\d+$/.test(val) ? "repeat(" + val + ", minmax(0, 1fr))" : val]];
    case "grid": return [["display", "grid"], ["grid-template-columns", val]];
    case "flex": {
      const l = val.toLowerCase();
      if (l === "row") return [["display", "flex"], ["flex-direction", "row"]];
      if (l === "col" || l === "column") return [["display", "flex"], ["flex-direction", "column"]];
      return [["flex", val]];
    }
    case "center": {
      const l = val.toLowerCase();
      if (l === "true" || l === "all" || l === "yes") return [["display", "flex"], ["align-items", "center"], ["justify-content", "center"]];
      return [["text-align", "center"]];
    }
    case "shadow": {
      const p = { sm: "0 1px 3px rgba(24,26,31,0.08)", md: "0 6px 20px rgba(24,26,31,0.08)", lg: "0 16px 36px rgba(24,26,31,0.12)" };
      return [["box-shadow", p[val.toLowerCase()] || val]];
    }
    default: return [[prop, val]];
  }
}
function __dogExpandMedia(q) {
  const map = { mobile: "(max-width: 640px)", tablet: "(max-width: 960px)", desktop: "(min-width: 1024px)", dark: "(prefers-color-scheme: dark)" };
  return map[String(q).toLowerCase()] || (String(q).startsWith("(") ? String(q) : "(" + q + ")");
}
function __dogBuildInlineCss() {
  const chunks = [];
  if (__dogCssVars.length) {
    chunks.push(":root {\\n" + __dogCssVars.map(([k, v]) => "  " + k + ": " + v + ";").join("\\n") + "\\n}");
  }
  for (const r of __dogStyleRules) {
    const body = r.decls.map(([p, v]) => "  " + p + ": " + v + ";").join("\\n");
    const block = r.selector + " {\\n" + body + "\\n}";
    chunks.push(r.media ? "@media " + __dogExpandMedia(r.media) + " {\\n" + block + "\\n}" : block);
  }
  return chunks.join("\\n\\n");
}
function __dogFlushStyles() {
  if (typeof document === "undefined" || (!__dogCssVars.length && !__dogStyleRules.length)) return;
  let el = document.getElementById("dog-inline-styles");
  if (!el) {
    el = document.createElement("style");
    el.id = "dog-inline-styles";
    (document.head || document.documentElement).appendChild(el);
  }
  el.textContent = __dogBuildInlineCss();
}
const __dogWeb = {
  toText(v) {
    return (typeof v === "object" && v !== null) ? JSON.stringify(v) : String(v);
  },
  print(v) {
    const line = __dogWeb.toText(v);
    console.log(line);
    if (typeof window !== "undefined" && window.parent && window.parent !== window) {
      try { window.parent.postMessage({ source: "dog-web-preview", type: "log", line }, "*"); } catch {}
    }
  },
  styleVar(name, val) {
    const key = "--" + String(name).trim().replace(/^--+/, "");
    const v = __dogExpandVarRefs(String(val).trim());
    const found = __dogCssVars.find((p) => p[0] === key);
    if (found) found[1] = v;
    else __dogCssVars.push([key, v]);
    __dogFlushStyles();
  },
  styleRule(selector, props) {
    const raw = String(selector).trim();
    const sel = raw === "theme" || raw === "root" ? ":root" : raw;
    const decls = [];
    for (const [k, v] of Object.entries(props || {})) {
      if (sel === ":root" && !String(k).trim().startsWith("--")) {
        __dogWeb.styleVar(k, v);
      } else {
        for (const pair of __dogExpandDecl(k, v)) decls.push(pair);
      }
    }
    if (decls.length) __dogStyleRules.push({ media: null, selector: sel, decls });
    __dogFlushStyles();
  },
  styleMedia(query, selector, props) {
    const sel = String(selector).trim();
    const decls = [];
    for (const [k, v] of Object.entries(props || {})) {
      for (const pair of __dogExpandDecl(k, v)) decls.push(pair);
    }
    if (decls.length) __dogStyleRules.push({ media: String(query).trim(), selector: sel, decls });
    __dogFlushStyles();
  },
  cssBundle() {
    return __dogBuildInlineCss();
  },
  dogCss(dsl) {
    const out = []; let mode = null; let sel = ""; let decls = [];
    for (const raw of String(dsl || "").split(/\\r?\\n/)) {
      const ln = raw.trim(); if (!ln || ln.startsWith("#") || ln.startsWith("//")) continue;
      if (ln === "end" || ln === "}") { if (mode === "vars") out.push(":root {\\n" + decls.join("\\n") + "\\n}"); else if (mode === "rule") out.push(sel + " {\\n" + decls.join("\\n") + "\\n}"); mode = null; sel = ""; decls = []; continue; }
      if (ln === "vars") { mode = "vars"; decls = []; continue; }
      if (ln.startsWith("rule ")) { mode = "rule"; sel = ln.slice(5).trim(); decls = []; continue; }
      const idx = ln.indexOf(":"); if (idx > 0 && mode) {
        const k = ln.slice(0, idx).trim(); const v = __dogExpandVarRefs(ln.slice(idx + 1).trim());
        if (mode === "vars") decls.push("  --" + k.replace(/^--+/, "") + ": " + v + ";");
        else { for (const [ek, ev] of __dogExpandDecl(k, v)) decls.push("  " + ek + ": " + ev + ";"); }
      }
    }
    return out.join("\\n\\n");
  },
  cssTheme(bg, surface, ink, accent, font) {
    return ":root{--bg:" + bg + ";--surface:" + surface + ";--ink:" + ink + ";--accent:" + accent + ";--font:" + font + ";}*{box-sizing:border-box;}body{margin:0;background:var(--bg);color:var(--ink);font-family:var(--font);line-height:1.6;}.dg-nav{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:14px 24px;background:var(--surface);border-bottom:1px solid rgba(128,128,128,0.2);}.dg-nav-links{display:flex;gap:18px;flex-wrap:wrap;}.dg-nav a{color:inherit;text-decoration:none;font-weight:500;}.dg-card{background:var(--surface);border:1px solid rgba(128,128,128,0.22);border-radius:10px;padding:20px;}.dg-grid{display:grid;gap:18px;}.dg-btn{cursor:pointer;border:none;border-radius:6px;padding:10px 16px;font-weight:600;background:var(--accent);color:#fff;}.dg-input{width:100%;padding:10px 12px;border-radius:6px;border:1px solid rgba(128,128,128,0.35);background:var(--surface);color:var(--ink);}";
  },
  cssRule(sel, props) {
    const lines = [];
    for (const [k, v] of Object.entries(props || {})) {
      for (const [ek, ev] of __dogExpandDecl(k, v)) lines.push("  " + ek + ": " + ev + ";");
    }
    return sel + " {\\n" + lines.join("\\n") + "\\n}";
  },
  cssVars(vars) {
    const lines = Object.entries(vars || {}).map(([k, v]) => "  --" + String(k).replace(/^--/, "") + ": " + v + ";");
    return ":root {\\n" + lines.join("\\n") + "\\n}";
  },
  cssMedia(q, css) { return "@media " + __dogExpandMedia(q) + " {\\n" + css + "\\n}"; },
  cssKeyframes(name, steps) { return "@keyframes " + name + " {" + Object.entries(steps || {}).map(([s, d]) => s + "{" + d + "}").join(" ") + "}"; },
  htmlEl(tag, attrs, content) {
    const a = Object.entries(attrs || {}).map(([k, v]) => " " + k + '=\\"' + __dogEscape(v) + '\\"').join("");
    if (["br","hr","img","input","meta","link"].includes(String(tag).toLowerCase())) return "<" + tag + a + " />";
    return "<" + tag + a + ">" + content + "</" + tag + ">";
  },
  htmlPage(title, css, body) {
    return "<!doctype html><html><head><title>" + __dogEscape(title) + "</title><style>" + css + "</style></head><body>" + body + "</body></html>";
  },
  webNav(brand, links) {
    const a = Object.entries(links || {}).map(([k, href]) => '<a href="' + __dogEscape(href) + '">' + __dogEscape(k) + "</a>").join("");
    return '<header class="dg-nav"><strong>' + __dogEscape(brand) + '</strong><nav class="dg-nav-links">' + a + "</nav></header>";
  },
  webCard(title, body) { return '<article class="dg-card"><h3>' + __dogEscape(title) + "</h3><div>" + body + "</div></article>"; },
  webGrid(cols, items) { return '<div class="dg-grid" style="grid-template-columns:repeat(' + cols + ',minmax(0,1fr));">' + (items || []).join("\\n") + "</div>"; },
  webButton(id, label) { return '<button id="' + __dogEscape(id) + '" class="dg-btn" type="button">' + __dogEscape(label) + "</button>"; },
  webInput(id, ph, val) { return '<input id="' + __dogEscape(id) + '" class="dg-input" type="text" placeholder="' + __dogEscape(ph) + '" value="' + __dogEscape(val) + '" />'; },
  webBadge(label, tone) { return '<span class="dg-badge dg-badge-' + __dogEscape(tone || "neutral") + '">' + __dogEscape(label) + "</span>"; },
  webAlert(title, msg, tone) { return '<div class="dg-alert dg-alert-' + __dogEscape(tone || "info") + '" role="' + (tone === "error" ? "alert" : "status") + '"><strong>' + __dogEscape(title) + '</strong>: ' + __dogEscape(msg) + "</div>"; },
  webField(id, label, ph, typ) { return '<div class="dg-field"><label class="dg-label" for="' + __dogEscape(id) + '">' + __dogEscape(label) + '</label><input id="' + __dogEscape(id) + '" class="dg-input" type="' + __dogEscape(typ || "text") + '" placeholder="' + __dogEscape(ph || "") + '" /></div>'; },
  webForm(id, inner, btnLabel) { return '<form id="' + __dogEscape(id) + '" class="dg-form" novalidate>' + String(inner || "") + '<button type="submit" class="dg-btn">' + __dogEscape(btnLabel || "Submit") + "</button></form>"; },
  webValidateRequired(sel) { const el = __dogQ(sel); const ok = Boolean(el && String(el.value || "").trim().length > 0); if (el && el.setAttribute) el.setAttribute("aria-invalid", String(!ok)); return ok; },
  webValidateEmail(sel) { const el = __dogQ(sel); const v = el ? String(el.value || "").trim() : ""; const ok = /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(v); if (el && el.setAttribute) el.setAttribute("aria-invalid", String(!ok)); return ok; },
  domFocus(sel) { const el = __dogQ(sel); if (el && el.focus) el.focus(); },
  domSetAttr(sel, attr, val) { const el = __dogQ(sel); if (el && el.setAttribute) el.setAttribute(String(attr), String(val)); },
  domMount(sel, html) {
    const el = __dogQ(sel) || (typeof document !== "undefined" ? document.body : null);
    if (el) el.innerHTML = String(html);
  },
  domSetText(sel, val) { const el = __dogQ(sel); if (el) el.textContent = String(val); },
  domSetHtml(sel, html) { const el = __dogQ(sel); if (el) el.innerHTML = String(html); },
  domGetValue(sel) { const el = __dogQ(sel); return el && "value" in el ? String(el.value) : ""; },
  domSetValue(sel, val) { const el = __dogQ(sel); if (el && "value" in el) el.value = String(val); },
  domAddClass(sel, cls) { const el = __dogQ(sel); if (el && el.classList) el.classList.add(String(cls)); },
  domRemoveClass(sel, cls) { const el = __dogQ(sel); if (el && el.classList) el.classList.remove(String(cls)); },
  domToggleClass(sel, cls) { const el = __dogQ(sel); return el && el.classList ? Boolean(el.classList.toggle(String(cls))) : false; },
  domSetStyle(sel, prop, val) { const el = __dogQ(sel); if (el && el.style) el.style[prop] = String(val); },
  domOn(sel, ev, fnName) {
    const el = __dogQ(sel);
    if (!el) return;
    el.addEventListener(String(ev), (e) => {
      if (String(ev) === "submit" && e && typeof e.preventDefault === "function") e.preventDefault();
      const fn = __dogFns[fnName];
      if (typeof fn === "function") fn();
    });
  },
  webTitle(t) { if (typeof document !== "undefined") document.title = String(t); },
  webComponent(name, fnName) { if (!this._components) this._components = Object.create(null); this._components[String(name)] = String(fnName); },
  webRenderComponent(name, props) {
    const fnName = this._components && this._components[String(name)];
    const fn = fnName ? __dogFns[fnName] : __dogFns[String(name)];
    return typeof fn === "function" ? String(fn(props || {})) : "";
  },
  webStateSet(k, v) {
    if (!this._state) this._state = Object.create(null);
    if (!this._watchers) this._watchers = Object.create(null);
    this._state[String(k)] = v;
    const list = this._watchers[String(k)] || [];
    for (const fnName of list) { const fn = __dogFns[fnName]; if (typeof fn === "function") fn(v); }
    return v;
  },
  webStateGet(k, fallback) {
    if (!this._state) return fallback;
    return Object.prototype.hasOwnProperty.call(this._state, String(k)) ? this._state[String(k)] : fallback;
  },
  webOnState(k, fnName) {
    if (!this._watchers) this._watchers = Object.create(null);
    const key = String(k);
    if (!this._watchers[key]) this._watchers[key] = [];
    this._watchers[key].push(String(fnName));
  },
  domDelegate(rootSel, childSel, ev, fnName) {
    const root = __dogQ(rootSel) || (typeof document !== "undefined" ? document : null);
    if (!root || !root.addEventListener) return;
    root.addEventListener(String(ev), (e) => {
      const target = e.target && e.target.closest ? e.target.closest(String(childSel)) : null;
      if (target) {
        const fn = __dogFns[fnName];
        if (typeof fn === "function") fn(target.dataset ? (target.dataset.tab || target.id || "") : "");
      }
    });
  },
  webTabs(id, tabsMap, activeKey) {
    const entries = Object.entries(tabsMap || {});
    const active = activeKey || (entries[0] ? entries[0][0] : "");
    const btns = entries.map(([k, lbl]) => '<button type="button" class="dg-tab-btn' + (k === active ? ' is-active' : '') + '" data-tab="' + __dogEscape(k) + '" role="tab" aria-selected="' + String(k === active) + '">' + __dogEscape(lbl) + '</button>').join("");
    return '<div id="' + __dogEscape(id) + '" class="dg-tabs" role="tablist">' + btns + '</div>';
  },
  webModal(id, title, body) {
    return '<dialog id="' + __dogEscape(id) + '" class="dg-modal" aria-label="' + __dogEscape(title) + '"><div class="dg-modal-head"><strong>' + __dogEscape(title) + '</strong></div><div class="dg-modal-body">' + String(body || "") + '</div></dialog>';
  },
  webTable(headers, rows) {
    const ths = (Array.isArray(headers) ? headers : []).map((h) => '<th scope="col">' + __dogEscape(h) + '</th>').join("");
    const trs = (Array.isArray(rows) ? rows : []).map((r) => '<tr>' + (Array.isArray(r) ? r : [r]).map((c) => '<td>' + __dogEscape(c) + '</td>').join("") + '</tr>').join("");
    return '<table class="dg-table"><thead><tr>' + ths + '</tr></thead><tbody>' + trs + '</tbody></table>';
  },
  webProgress(id, value, max) {
    return '<progress id="' + __dogEscape(id) + '" class="dg-progress" value="' + __dogEscape(value) + '" max="' + __dogEscape(max || 100) + '"></progress>';
  },
  webToast(msg, tone) {
    return '<div class="dg-toast dg-toast-' + __dogEscape(tone || "info") + '" role="status">' + __dogEscape(msg) + '</div>';
  },
  domAttr(sel, attr) {
    const el = __dogQ(sel);
    return el && el.getAttribute ? (el.getAttribute(String(attr)) || "") : "";
  },
  webStorageSet(k, v) { try { localStorage.setItem(String(k), String(v)); } catch { __dogStorageFallback[k] = String(v); } },
  webStorageGet(k) { try { return localStorage.getItem(String(k)) || ""; } catch { return __dogStorageFallback[k] || ""; } },
  webRoute() { return typeof location !== "undefined" ? (location.hash || location.pathname || "/") : "/"; },
  webNavigate(p) { if (typeof location !== "undefined") location.hash = String(p).startsWith("#") ? String(p) : "#" + String(p); }
};`;

function escapeHtml(s) {
  return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function compileDogWebsite({ code, css = "", title = "DOG Web App", minify = false, treeShake = false }) {
  try {
    const rawDgss = css.trim() ? compileDogCss(css, { minify: false }) : "";
    const userJs = compileDogSourceToJs(code);
    const fullJs = `${RUNTIME_HEADER}\n\n${userJs}\n__dogFlushStyles();`;

    let inlineCss = "";
    try {
      const collectFn = new Function(
        "document",
        "window",
        "location",
        "localStorage",
        "console",
        `${RUNTIME_HEADER}\n${userJs}\nreturn __dogBuildInlineCss();`
      );
      const noopConsole = { log() {}, warn() {}, error() {}, info() {}, clear() {} };
      const memStorage = { getItem() { return null; }, setItem() {} };
      inlineCss = String(collectFn(undefined, undefined, undefined, memStorage, noopConsole) || "");
    } catch {
      inlineCss = "";
    }

    let combinedCss = [rawDgss, inlineCss].filter((s) => s && s.trim().length > 0).join("\n\n");
    let prunedSelectors = [];
    if (treeShake && combinedCss) {
      const shaken = treeShakeDogCss(combinedCss, `${code}\n${userJs}\ndg-nav dg-nav-links dg-card dg-grid dg-btn dg-input dg-badge dg-alert dg-field dg-form dg-tabs dg-tab-btn dg-modal dg-table dg-progress dg-toast`);
      combinedCss = shaken.css;
      prunedSelectors = shaken.prunedSelectors;
    }
    const compiledCss = minify && combinedCss ? minifyDogCss(combinedCss) : combinedCss;
    const diags = analyzeWebCompatibilityAndA11y({ code, css: combinedCss, userJs, prunedCount: prunedSelectors.length });
    const safeJs = fullJs.replace(/<\/script/gi, "<\\/script");
    const indexHtml = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>${escapeHtml(title)}</title>
  <style id="dog-style-sheet">
${compiledCss}
  </style>
</head>
<body>
  <div id="app"></div>
  <script>
${safeJs}
  </script>
</body>
</html>`;
    return {
      ok: true,
      css: compiledCss,
      js: userJs,
      runtimeJs: fullJs,
      indexHtml,
      prunedSelectors,
      diagnostics: diags,
      diagnosticsJson: JSON.stringify({
        ok: true,
        engine: "dog-web/1.0.0",
        minified: Boolean(minify),
        treeShaken: Boolean(treeShake),
        prunedSelectors,
        diagnostics: diags,
      }, null, 2),
    };
  } catch (err) {
    if (err instanceof DogWebDiagnostic) {
      return {
        ok: false,
        errorCode: err.code,
        error: err.format(),
        diagnosticsJson: JSON.stringify({ ok: false, engine: "dog-web/1.0.0", diagnostics: [err.toJSON()] }, null, 2),
        css: "",
        js: "",
        indexHtml: "",
      };
    }
    return {
      ok: false,
      errorCode: "E004",
      error: String(err.message || err),
      diagnosticsJson: JSON.stringify({ ok: false, engine: "dog-web/1.0.0", diagnostics: [{ code: "E004", severity: "error", line: 1, title: String(err.message || err) }] }, null, 2),
      css: "",
      js: "",
      indexHtml: "",
    };
  }
}
