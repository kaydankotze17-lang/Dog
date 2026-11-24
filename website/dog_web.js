// Pure DOG Web (.dg -> JavaScript) + DOG CSS (.dgss -> CSS) Compiler
// Shared between the official website's Web & CSS Studio and automated node:test suites.

export class DogWebDiagnostic extends Error {
  constructor({ code, title, line, why, fix }) {
    super(`[${code}] line ${line || 1}: ${title}`);
    this.code = code || "E004";
    this.title = title;
    this.line = line || 1;
    this.why = why || "";
    this.fix = fix || "";
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
    default:
      return [[prop, val]];
  }
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

export function compileDogCss(dsl) {
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
      if (block.decls.length > 0) {
        const declLines = [];
        for (const [k, v] of block.decls) {
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
  return out.join("\n\n");
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
    if (["==", "!=", ">=", "<="].includes(two)) {
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
    if (["text", "lower", "upper", "trim", "join", "dog_css", "css_rule", "css_vars", "css_media", "css_keyframes", "css_theme", "css_bundle", "html_el", "html_page", "web_nav", "web_card", "web_grid", "web_button", "web_input", "dom_get_value", "web_storage_get", "web_route", "json_stringify", "html_escape"].includes(node.callee)) {
      return "text";
    }
    if (["number", "len"].includes(node.callee)) return "number";
    if (["contains", "dom_toggle_class"].includes(node.callee)) return "bool";
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
          args.push(parseOr());
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
        const index = parseOr();
        if (peek()?.type !== "punct" || peek().value !== "]") {
          throw new DogWebDiagnostic({
            code: "E004",
            title: "Expected ']' after index.",
            line,
            why: "Unclosed bracket index.",
            fix: "Add closing `]`.",
          });
        }
        pos++;
        expr = { kind: "index", target: expr, index, line };
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
    if (t.type === "id") {
      pos++;
      return { kind: "id", name: t.value, line };
    }
    if (t.type === "punct" && t.value === "(") {
      pos++;
      const inner = parseOr();
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
        elements.push(parseOr());
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
        const k = parseOr();
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
        const v = parseOr();
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

  return parseOr();
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
    case "list":
      return `[${node.elements.map(emitJsFromNode).join(", ")}]`;
    case "map":
      return `({ ${node.entries.map(([k, v]) => `[${emitJsFromNode(k)}]: ${emitJsFromNode(v)}`).join(", ")} })`;
    case "unary":
      return `(${node.op}(${emitJsFromNode(node.expr)}))`;
    case "bin":
      return `(${emitJsFromNode(node.left)} ${node.op} ${emitJsFromNode(node.right)})`;
    case "call": {
      const a = node.args.map(emitJsFromNode);
      const mapCall = {
        say: () => `__dogWeb.print(${a[0]})`,
        print: () => `__dogWeb.print(${a[0]})`,
        cli_info: () => `__dogWeb.print(${a[0]})`,
        cli_success: () => `__dogWeb.print(${a[0]})`,
        cli_warn: () => `__dogWeb.print(${a[0]})`,
        cli_error: () => `__dogWeb.print(${a[0]})`,
        text: () => `String(${a[0]})`,
        number: () => `parseInt(String(${a[0]}), 10)`,
        decimal: () => `Number(${a[0]})`,
        lower: () => `String(${a[0]}).toLowerCase()`,
        upper: () => `String(${a[0]}).toUpperCase()`,
        trim: () => `String(${a[0]}).trim()`,
        contains: () => `String(${a[0]}).includes(String(${a[1]}))`,
        split: () => `String(${a[0]}).split(String(${a[1]}))`,
        join: () => `(${a[0]}).join(String(${a[1]}))`,
        len: () => `(Array.isArray(${a[0]}) || typeof ${a[0]} === "string" ? ${a[0]}.length : Object.keys(${a[0]} || {}).length)`,
        push: () => `(${a[0]}).push(${a[1]})`,
        pop: () => `(${a[0]}).pop()`,
        clear: () => (a.length === 0 ? "console.clear()" : `(${a[0]}.length = 0)`),
        range: () => `Array.from({ length: Math.max(0, (${a[1]}) - (${a[0]})) }, (_, i) => (${a[0]}) + i)`,
        map_get: () => `(${a[0]})[${a[1]}]`,
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
  const lines = rawLines.map((raw, idx) => ({
    line: idx + 1,
    text: stripDgComment(raw).trim(),
  })).filter((l) => l.text.length > 0);

  let cursor = 0;

  function parseBlock(stopTokens) {
    const out = [];
    while (cursor < lines.length) {
      const { line, text } = lines[cursor];
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
        fix: "Add closing `end` for each open block (`define`, `if`, `while`, `for`, `repeat`, `try`).",
      });
    }
    return out;
  }

  function parseStmt() {
    const { line, text } = lines[cursor++];

    if (text.startsWith("use ")) {
      return `// ${text}`;
    }
    if (text === "break" || text === "continue") {
      return `${text};`;
    }
    if (text === "return") {
      return "return;";
    }
    if (text.startsWith("return ")) {
      const node = parseExprNode(text.slice(7).trim(), line, envTypes);
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

    // Bindings: let / var / const
    const bindMatch = text.match(/^(let|var|const)\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*:\s*[^=]+)?\s*=\s*(.+)$/);
    if (bindMatch) {
      const [, kw, name, exprStr] = bindMatch;
      const node = parseExprNode(exprStr, line, envTypes);
      envTypes.set(name, inferRoughType(node, envTypes));
      const jsKw = kw === "var" ? "let" : "const";
      return `${jsKw} ${name} = ${emitJsFromNode(node)};`;
    }

    // Natural English assignments
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

    // Compound assignment & direct assignment
    const compMatch = text.match(/^([a-zA-Z_][a-zA-Z0-9_.\[\]]*)\s*(\+=|-=|\*=|\/=|%=|=)\s*(.+)$/);
    if (compMatch) {
      const [, lhs, op, rhs] = compMatch;
      const node = parseExprNode(rhs, line, envTypes);
      return `${lhs} ${op} ${emitJsFromNode(node)};`;
    }

    // Functions: define / fn / function
    const fnMatch = text.match(/^(?:define|fn|function)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)(?:\s*(?:returns|->)\s*(.+))?$/);
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

    // if / unless
    if (text.startsWith("if ") || text.startsWith("unless ")) {
      const isUnless = text.startsWith("unless ");
      const rawCond = (isUnless ? text.slice(7) : text.slice(3)).replace(/\s+then$/, "").trim();
      const condNode = parseExprNode(rawCond, line, envTypes);
      const condJs = isUnless ? `!(${emitJsFromNode(condNode)})` : emitJsFromNode(condNode);
      const branches = [{ condJs, body: parseBlock(["elif", "else", "otherwise", "end"]) }];

      while (cursor < lines.length && lines[cursor].text !== "end") {
        const kwLine = lines[cursor++];
        const kt = kwLine.text;
        if (kt.startsWith("elif ")) {
          const c = kt.slice(5).replace(/\s+then$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "else", "otherwise", "end"]),
          });
        } else if (kt.startsWith("otherwise if ")) {
          const c = kt.slice(13).replace(/\s+then$/, "").trim();
          branches.push({
            condJs: emitJsFromNode(parseExprNode(c, kwLine.line, envTypes)),
            body: parseBlock(["elif", "else", "otherwise", "end"]),
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
          why: "Open `if` / `unless` block has no closing `end`.",
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
      const rawCond = (isUntil ? text.slice(6) : text.slice(6)).replace(/\s+then$/, "").trim();
      const condJs = emitJsFromNode(parseExprNode(rawCond, line, envTypes));
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after loop.", line, why: "Loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `while (${isUntil ? `!(${condJs})` : condJs}) {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // repeat N times
    if (text.startsWith("repeat ")) {
      const rawCount = text.slice(7).replace(/\s+times$/, "").trim();
      const countJs = emitJsFromNode(parseExprNode(rawCount, line, envTypes));
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after `repeat`.", line, why: "Repeat loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `for (let __rep = 0; __rep < (${countJs}); __rep++) {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // for i from A to B | for each x in list
    if (text.startsWith("for ")) {
      const fromMatch = text.match(/^for\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
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
            fix: "Use `for i from 1 to 5` or `for each item in list`.",
          });
        }
        const [, varName, listExpr] = eachMatch;
        headJs = `for (const ${varName} of ${emitJsFromNode(parseExprNode(listExpr, line, envTypes))})`;
      }
      const body = parseBlock(["end"]);
      if (lines[cursor]?.text !== "end") {
        throw new DogWebDiagnostic({ code: "E005", title: "Missing `end` after `for` loop.", line, why: "For loop is unclosed.", fix: "Add `end`." });
      }
      cursor++;
      return `${headJs} {\n${body.map((s) => `  ${s}`).join("\n")}\n}`;
    }

    // try ... catch err ... end
    if (text === "try") {
      const tryBody = parseBlock(["catch", "end"]);
      let catchVar = "err";
      let catchBody = [];
      if (lines[cursor]?.text.startsWith("catch ")) {
        catchVar = lines[cursor].text.slice(6).trim() || "err";
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
    el.addEventListener(String(ev), () => {
      const fn = __dogFns[fnName];
      if (typeof fn === "function") fn();
    });
  },
  webTitle(t) { if (typeof document !== "undefined") document.title = String(t); },
  webStorageSet(k, v) { try { localStorage.setItem(String(k), String(v)); } catch { __dogStorageFallback[k] = String(v); } },
  webStorageGet(k) { try { return localStorage.getItem(String(k)) || ""; } catch { return __dogStorageFallback[k] || ""; } },
  webRoute() { return typeof location !== "undefined" ? (location.hash || location.pathname || "/") : "/"; },
  webNavigate(p) { if (typeof location !== "undefined") location.hash = String(p).startsWith("#") ? String(p) : "#" + String(p); }
};`;

function escapeHtml(s) {
  return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function compileDogWebsite({ code, css = "", title = "DOG Web App" }) {
  try {
    const dgssCss = css.trim() ? compileDogCss(css) : "";
    const userJs = compileDogSourceToJs(code);
    const fullJs = `${RUNTIME_HEADER}\n\n${userJs}\n__dogFlushStyles();`;

    // Also harvest any inline `style ... end` rules produced at top level in an isolated scope
    // (shadowing browser globals so compiling never mutates the host studio page).
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

    const compiledCss = [dgssCss, inlineCss].filter((s) => s && s.trim().length > 0).join("\n\n");
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
    return { ok: true, css: compiledCss, js: userJs, runtimeJs: fullJs, indexHtml };
  } catch (err) {
    if (err instanceof DogWebDiagnostic) {
      return {
        ok: false,
        errorCode: err.code,
        error: err.format(),
        css: "",
        js: "",
        indexHtml: "",
      };
    }
    return {
      ok: false,
      errorCode: "E004",
      error: String(err.message || err),
      css: "",
      js: "",
      indexHtml: "",
    };
  }
}
