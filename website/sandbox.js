// Pure browser/Node subset evaluator & strict type diagnostic engine for DOG v0.3.0
// Mirrors DOG's English + classic syntax, strict typing (no implicit text + number), and block rules.

const MAX_STEPS = 8000;

export class DogRuntimeError extends Error {
  constructor({ code, title, line, why, fix, tip }) {
    super(`${code}: ${title}`);
    this.code = code;
    this.title = title;
    this.line = line || 1;
    this.why = why || "";
    this.fix = fix || "";
    this.tip = tip || "";
  }

  format() {
    return [
      `[${this.code}] line ${this.line}: ${this.title}`,
      this.why ? `WHY: ${this.why}` : "",
      this.fix ? `FIX: ${this.fix}` : "",
      this.tip ? `TIP: ${this.tip}` : "",
    ]
      .filter(Boolean)
      .join("\n");
  }
}

function stripComment(raw) {
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
    if (ch === "#" || (ch === "/" && raw[i + 1] === "/")) {
      return raw.slice(0, i);
    }
  }
  return raw;
}

export function tokenizeExpression(src, lineNum = 1) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (ch === " " || ch === "\t" || ch === "\r") {
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
          if (lit.length > 0) {
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
            throw new DogRuntimeError({
              code: "E002",
              title: "Unclosed '{' inside interpolated string.",
              line: lineNum,
              why: "Interpolation placeholder `{...}` was not closed with '}'.",
              fix: 'Close braces inside $"...", e.g. $"Hi {name}".',
            });
          }
          parts.push({ type: "expr", value: exprStr.trim() });
          continue;
        }
        lit += c;
        i++;
      }
      if (!closed) {
        throw new DogRuntimeError({
          code: "E002",
          title: "Unterminated interpolated string literal.",
          line: lineNum,
          why: 'String opened with $" was not closed before the end of the line.',
          fix: 'Add a closing `"` at the end of the string.',
        });
      }
      if (lit.length > 0 || parts.length === 0) {
        parts.push({ type: "lit", value: lit });
      }
      tokens.push({ kind: "interp", value: parts });
      continue;
    }
    if (ch === '"') {
      i++;
      let s = "";
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
          if (next === "n") s += "\n";
          else if (next === "t") s += "\t";
          else if (next !== undefined) s += next;
          continue;
        }
        s += c;
        i++;
      }
      if (!closed) {
        throw new DogRuntimeError({
          code: "E002",
          title: "Unterminated string literal.",
          line: lineNum,
          why: 'A string opened with `"` was not closed.',
          fix: 'Add a matching closing `"` quote.',
        });
      }
      tokens.push({ kind: "string", value: s });
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let raw = "";
      let isDec = false;
      while (i < src.length && (/[0-9_]/.test(src[i]) || (src[i] === "." && !isDec && /[0-9]/.test(src[i + 1] || "")))) {
        if (src[i] === ".") isDec = true;
        raw += src[i];
        i++;
      }
      const clean = raw.replace(/_/g, "");
      tokens.push({ kind: isDec ? "decimal" : "number", value: isDec ? Number(clean) : parseInt(clean, 10) });
      continue;
    }
    if (/[a-zA-Z_]/.test(ch)) {
      let ident = "";
      while (i < src.length && /[a-zA-Z0-9_]/.test(src[i])) {
        ident += src[i];
        i++;
      }
      if (ident === "true" || ident === "false") {
        tokens.push({ kind: "bool", value: ident === "true" });
      } else {
        tokens.push({ kind: "ident", value: ident });
      }
      continue;
    }
    const two = src.slice(i, i + 2);
    if (["==", "!=", ">=", "<="].includes(two)) {
      tokens.push({ kind: "op", value: two });
      i += 2;
      continue;
    }
    if (ch === "!" && src[i + 1] !== "=") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '!'.",
        line: lineNum,
        why: "DOG uses English logical words instead of C-style '!' negation.",
        fix: "Write `not condition` instead of `!condition` (keep `!=` for not-equal).",
        tip: "DOG reads like English: `if not ready` or `if name is not \"\"`.",
      });
    }
    if (ch === "&") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '&'.",
        line: lineNum,
        why: "DOG uses the English word 'and' instead of '&' or '&&'.",
        fix: "Replace `&&` with `and`, e.g. `if a > 0 and b > 0`.",
      });
    }
    if (ch === "|") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '|'.",
        line: lineNum,
        why: "DOG uses the English word 'or' instead of '|' or '||'.",
        fix: "Replace `||` with `or`, e.g. `if role is \"admin\" or role is \"mod\"`.",
      });
    }
    if (ch === ";") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character ';'.",
        line: lineNum,
        why: "DOG uses newlines to end statements and does not require semicolons.",
        fix: "Remove the trailing `;` at the end of the line.",
      });
    }
    if ("+-*/%><()[],.:{}".includes(ch)) {
      tokens.push({ kind: "op", value: ch });
      i++;
      continue;
    }
    throw new DogRuntimeError({
      code: "E001",
      title: `Unexpected character '${ch}'.`,
      line: lineNum,
      why: "Encountered syntax not valid in DOG.",
      fix: "Remove stray punctuation or wrap text in double quotes.",
    });
  }
  return tokens;
}

function typeNameOf(val) {
  if (val === undefined || val === null) return "void";
  if (typeof val === "string") return "text";
  if (typeof val === "boolean") return "bool";
  if (typeof val === "number") return Number.isInteger(val) ? "number" : "decimal";
  if (Array.isArray(val)) return "list";
  if (typeof val === "object" && val.__dogType) return val.__dogType;
  if (typeof val === "object") return "map";
  return "text";
}

function makeTypedVal(kind, value) {
  if (kind === "decimal") {
    return { __dogType: "decimal", value: Number(value) };
  }
  return value;
}

function unwrapNum(v) {
  if (v && typeof v === "object" && v.__dogType === "decimal") return { isDecimal: true, n: v.value };
  if (typeof v === "number") return { isDecimal: !Number.isInteger(v), n: v };
  return null;
}

export function formatDogValue(v) {
  if (v && typeof v === "object" && v.__dogType === "decimal") {
    const s = String(v.value);
    return s.includes(".") ? s : `${s}.0`;
  }
  if (Array.isArray(v)) return `[${v.map(formatDogValue).join(", ")}]`;
  if (v && typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function dogEquals(a, b, lineNum) {
  const ta = typeNameOf(a);
  const tb = typeNameOf(b);
  if (ta !== tb) {
    throw new DogRuntimeError({
      code: "E021",
      title: "Type mismatch detected.",
      line: lineNum,
      why: `Cannot compare '${ta}' with '${tb}' without explicit conversion.`,
      fix: "Convert one side with `text()`, `number()`, or `decimal()` so both sides match.",
      tip: "DOG prevents silent comparisons across mismatched types.",
    });
  }
  const na = unwrapNum(a);
  const nb = unwrapNum(b);
  if (na && nb) return na.n === nb.n;
  return JSON.stringify(a) === JSON.stringify(b);
}

function evalExprString(exprSrc, ctx, lineNum) {
  const tokens = tokenizeExpression(exprSrc, lineNum);
  let pos = 0;
  const peek = (offset = 0) => tokens[pos + offset];
  const peekIdent = (offset = 0) => {
    const t = tokens[pos + offset];
    return t && t.kind === "ident" ? t.value : null;
  };
  const consume = () => tokens[pos++];

  function parseOr() {
    let left = parseAnd();
    while (peekIdent(0) === "or") {
      consume();
      const right = parseAnd();
      if (typeNameOf(left) !== "bool" || typeNameOf(right) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Logical 'or' requires bool operands.",
          line: lineNum,
          why: `Got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Ensure both sides of `or` evaluate to `true` or `false`.",
        });
      }
      left = left || right;
    }
    return left;
  }

  function parseAnd() {
    let left = parseEquality();
    while (peekIdent(0) === "and") {
      consume();
      const right = parseEquality();
      if (typeNameOf(left) !== "bool" || typeNameOf(right) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Logical 'and' requires bool operands.",
          line: lineNum,
          why: `Got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Ensure both sides of `and` evaluate to `true` or `false`.",
        });
      }
      left = left && right;
    }
    return left;
  }

  function parseEquality() {
    let left = parseComparison();
    while (true) {
      let op = null;
      let count = 0;
      const t = peek();
      if (t && t.kind === "op" && (t.value === "==" || t.value === "!=")) {
        op = t.value;
        count = 1;
      } else if (peekIdent(0) === "equals") {
        op = "==";
        count = 1;
      } else if (peekIdent(0) === "is") {
        if (peekIdent(1) === "not" && peekIdent(2) === "equal" && peekIdent(3) === "to") {
          op = "!=";
          count = 4;
        } else if (peekIdent(1) === "not") {
          op = "!=";
          count = 2;
        } else if (peekIdent(1) === "equal" && peekIdent(2) === "to") {
          op = "==";
          count = 3;
        } else {
          op = "==";
          count = 1;
        }
      }
      if (!op) break;
      for (let k = 0; k < count; k++) consume();
      const right = parseComparison();
      const eq = dogEquals(left, right, lineNum);
      left = op === "==" ? eq : !eq;
    }
    return left;
  }

  function parseComparison() {
    let left = parseTerm();
    while (true) {
      let op = null;
      let count = 0;
      const t = peek();
      if (t && t.kind === "op" && [">", "<", ">=", "<="].includes(t.value)) {
        op = t.value;
        count = 1;
      } else if (peekIdent(0) === "is") {
        if (peekIdent(1) === "greater" && peekIdent(2) === "than") {
          op = ">";
          count = 3;
        } else if (peekIdent(1) === "less" && peekIdent(2) === "than") {
          op = "<";
          count = 3;
        } else if (peekIdent(1) === "at" && peekIdent(2) === "least") {
          op = ">=";
          count = 3;
        } else if (peekIdent(1) === "at" && peekIdent(2) === "most") {
          op = "<=";
          count = 3;
        } else if (peekIdent(1) === "over") {
          op = ">";
          count = 2;
        } else if (peekIdent(1) === "under") {
          op = "<";
          count = 2;
        }
      }
      if (!op) break;
      for (let k = 0; k < count; k++) consume();
      const right = parseTerm();
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch in comparison.",
          line: lineNum,
          why: `Ordering comparisons require matching 'number' or 'decimal' operands, got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Convert text inputs explicitly with `number()` or `decimal()` before comparing.",
        });
      }
      if (op === ">") left = na.n > nb.n;
      else if (op === "<") left = na.n < nb.n;
      else if (op === ">=") left = na.n >= nb.n;
      else left = na.n <= nb.n;
    }
    return left;
  }

  function parseTerm() {
    let left = parseFactor();
    while (peek() && peek().kind === "op" && (peek().value === "+" || peek().value === "-")) {
      const op = consume().value;
      const right = parseFactor();
      if (op === "+" && typeof left === "string" && typeof right === "string") {
        left = left + right;
        continue;
      }
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        const lt = typeNameOf(left);
        const rt = typeNameOf(right);
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch detected.",
          line: lineNum,
          why: `Cannot apply '${op}' to '${lt}' and '${rt}' without explicit conversion.`,
          fix:
            op === "+"
              ? 'Use `$"text {value}"` interpolation or convert explicitly with `text()`, `number()`, or `decimal()`.'
              : "Ensure both operands are matching `number` or `decimal` values.",
          tip: "Strict typing prevents silent JavaScript-style coercion bugs.",
        });
      }
      const res = op === "+" ? na.n + nb.n : na.n - nb.n;
      left = makeTypedVal(na.isDecimal ? "decimal" : "number", res);
    }
    return left;
  }

  function parseFactor() {
    let left = parseUnary();
    while (peek() && peek().kind === "op" && ["*", "/", "%"].includes(peek().value)) {
      const op = consume().value;
      const right = parseUnary();
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch detected.",
          line: lineNum,
          why: `Arithmetic '${op}' requires matching numeric types, got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Keep operands matching (`number` with `number`, or `decimal` with `decimal`).",
        });
      }
      if ((op === "/" || op === "%") && nb.n === 0) {
        throw new DogRuntimeError({
          code: "E031",
          title: "Division by zero.",
          line: lineNum,
          why: "Cannot divide or take modulo by zero.",
          fix: "Check that the divisor is non-zero before dividing.",
        });
      }
      if (op === "*") {
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", na.n * nb.n);
      } else if (op === "/") {
        const val = na.isDecimal ? na.n / nb.n : Math.trunc(na.n / nb.n);
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", val);
      } else {
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", na.n % nb.n);
      }
    }
    return left;
  }

  function parseUnary() {
    if (peek() && peek().kind === "op" && peek().value === "-") {
      consume();
      const v = parseUnary();
      const nv = unwrapNum(v);
      if (!nv) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Unary '-' requires numeric value.",
          line: lineNum,
          why: `Got '${typeNameOf(v)}'.`,
          fix: "Apply '-' only to a `number` or `decimal`.",
        });
      }
      return makeTypedVal(nv.isDecimal ? "decimal" : "number", -nv.n);
    }
    if (peekIdent(0) === "not") {
      consume();
      const v = parseUnary();
      if (typeNameOf(v) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Unary 'not' requires a bool expression.",
          line: lineNum,
          why: `Got '${typeNameOf(v)}'.`,
          fix: "Pass a boolean expression to `not`.",
        });
      }
      return !v;
    }
    return parsePostfix();
  }

  function parsePostfix() {
    let val = parsePrimary();
    while (peek()) {
      if (peek().kind === "op" && peek().value === "[") {
        consume();
        const idx = parseOr();
        if (!peek() || peek().kind !== "op" || peek().value !== "]") {
          throw new DogRuntimeError({
            code: "E004",
            title: "Expected closing ']' in index expression.",
            line: lineNum,
          });
        }
        consume();
        if (!Array.isArray(val)) {
          throw new DogRuntimeError({
            code: "E021",
            title: "Indexing requires a list value.",
            line: lineNum,
            why: `Cannot index value of type '${typeNameOf(val)}'.`,
          });
        }
        const n = unwrapNum(idx);
        if (!n || n.isDecimal || n.n < 0 || n.n >= val.length) {
          throw new DogRuntimeError({
            code: "E032",
            title: `List index out of bounds.`,
            line: lineNum,
            why: `Index ${formatDogValue(idx)} is invalid for list of length ${val.length}.`,
          });
        }
        val = val[n.n];
      } else if (peek().kind === "op" && peek().value === ".") {
        consume();
        const fieldTok = consume();
        if (!fieldTok || fieldTok.kind !== "ident") {
          throw new DogRuntimeError({ code: "E004", title: "Expected field name after '.'.", line: lineNum });
        }
        if (val && typeof val === "object" && val.__module) {
          const nsName = `${val.__module}.${fieldTok.value}`;
          if (peek() && peek().kind === "op" && peek().value === "(") {
            const args = parseCallArgs();
            val = callNamespaceOrBuiltin(nsName, args, ctx, lineNum);
          } else {
            val = { __module: nsName };
          }
        } else if (val && typeof val === "object" && Object.prototype.hasOwnProperty.call(val, fieldTok.value)) {
          val = val[fieldTok.value];
        } else {
          throw new DogRuntimeError({
            code: "E010",
            title: `Unknown field '${fieldTok.value}'.`,
            line: lineNum,
          });
        }
      } else {
        break;
      }
    }
    return val;
  }

  function parseCallArgs() {
    consume(); // (
    const args = [];
    if (!(peek() && peek().kind === "op" && peek().value === ")")) {
      while (true) {
        args.push(parseOr());
        if (peek() && peek().kind === "op" && peek().value === ",") {
          consume();
        } else {
          break;
        }
      }
    }
    if (!peek() || peek().kind !== "op" || peek().value !== ")") {
      throw new DogRuntimeError({
        code: "E004",
        title: "Expected ')' after function arguments.",
        line: lineNum,
      });
    }
    consume(); // )
    return args;
  }

  function parsePrimary() {
    const tok = consume();
    if (!tok) {
      throw new DogRuntimeError({
        code: "E004",
        title: "Unexpected end of expression.",
        line: lineNum,
      });
    }
    if (tok.kind === "string" || tok.kind === "number" || tok.kind === "bool") {
      return tok.value;
    }
    if (tok.kind === "decimal") {
      return makeTypedVal("decimal", tok.value);
    }
    if (tok.kind === "interp") {
      return tok.value
        .map((p) => (p.type === "lit" ? p.value : formatDogValue(evalExprString(p.value, ctx, lineNum))))
        .join("");
    }
    if (tok.kind === "op" && tok.value === "(") {
      const inner = parseOr();
      if (!peek() || peek().kind !== "op" || peek().value !== ")") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing ')'.", line: lineNum });
      }
      consume();
      return inner;
    }
    if (tok.kind === "op" && tok.value === "[") {
      const items = [];
      if (!(peek() && peek().kind === "op" && peek().value === "]")) {
        while (true) {
          items.push(parseOr());
          if (peek() && peek().kind === "op" && peek().value === ",") consume();
          else break;
        }
      }
      if (!peek() || peek().kind !== "op" || peek().value !== "]") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing ']' in list.", line: lineNum });
      }
      consume();
      return items;
    }
    if (tok.kind === "op" && tok.value === "{") {
      const obj = {};
      if (!(peek() && peek().kind === "op" && peek().value === "}")) {
        while (true) {
          const k = String(parseOr());
          if (!peek() || peek().kind !== "op" || peek().value !== ":") {
            throw new DogRuntimeError({ code: "E004", title: "Expected ':' in map literal.", line: lineNum });
          }
          consume();
          obj[k] = parseOr();
          if (peek() && peek().kind === "op" && peek().value === ",") consume();
          else break;
        }
      }
      if (!peek() || peek().kind !== "op" || peek().value !== "}") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing '}' in map.", line: lineNum });
      }
      consume();
      return obj;
    }
    if (tok.kind === "ident") {
      if (tok.value === "std" && peek() && peek().kind === "op" && peek().value === ".") {
        return { __module: "std" };
      }
      if (peek() && peek().kind === "op" && peek().value === "(") {
        const args = parseCallArgs();
        return callNamespaceOrBuiltin(tok.value, args, ctx, lineNum);
      }
      const binding = ctx.lookup(tok.value);
      if (!binding) {
        throw new DogRuntimeError({
          code: "E010",
          title: `Undefined variable '${tok.value}'.`,
          line: lineNum,
          why: `No variable named '${tok.value}' exists in the current scope.`,
          fix: `Declare it first with \`let ${tok.value} = ...\` or \`var ${tok.value} = ...\`.`,
        });
      }
      return binding.value;
    }
    throw new DogRuntimeError({
      code: "E004",
      title: "Unexpected token in expression.",
      line: lineNum,
    });
  }

  const result = parseOr();
  if (pos < tokens.length) {
    throw new DogRuntimeError({
      code: "E004",
      title: `Unexpected trailing token '${tokens[pos].value}' in expression.`,
      line: lineNum,
    });
  }
  return result;
}

function callNamespaceOrBuiltin(rawName, args, ctx, lineNum) {
  const name = rawName.startsWith("std.") ? rawName.split(".").pop() : rawName;
  if (ctx.functions.has(name)) {
    const fn = ctx.functions.get(name);
    if (args.length > fn.params.length) {
      throw new DogRuntimeError({
        code: "E014",
        title: `Too many arguments for '${name}()'.`,
        line: lineNum,
      });
    }
    ctx.pushScope();
    try {
      for (let idx = 0; idx < fn.params.length; idx++) {
        const p = fn.params[idx];
        let argVal;
        if (idx < args.length) {
          argVal = args[idx];
        } else if (p.defaultExpr !== null) {
          argVal = evalExprString(p.defaultExpr, ctx, lineNum);
        } else {
          throw new DogRuntimeError({
            code: "E014",
            title: `Missing argument '${p.name}' for '${name}()'.`,
            line: lineNum,
          });
        }
        ctx.define(p.name, "let", argVal);
      }
      const flow = execBlock(fn.body, ctx);
      if (flow.type === "return") return flow.value;
      return undefined;
    } finally {
      ctx.popScope();
    }
  }

  switch (name) {
    case "say":
    case "print":
      ctx.output.push(formatDogValue(args[0]));
      return undefined;
    case "cli_info":
      ctx.output.push(`[INFO] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_success":
      ctx.output.push(`[OK] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_warn":
      ctx.output.push(`[WARN] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_error":
      ctx.output.push(`[ERR] ${formatDogValue(args[0])}`);
      return undefined;
    case "progress":
    case "cli_progress": {
      const label = String(args[0]);
      const cur = Number(args[1] || 0);
      const tot = Math.max(1, Number(args[2] || 1));
      const pct = Math.min(100, Math.round((cur / tot) * 100));
      const filled = Math.round((pct / 100) * 16);
      ctx.output.push(`${label} [${"#".repeat(filled)}${".".repeat(16 - filled)}] ${pct}% (${cur}/${tot})`);
      return undefined;
    }
    case "text":
      return formatDogValue(args[0]);
    case "number": {
      const n = Number(unwrapNum(args[0])?.n ?? args[0]);
      if (!Number.isFinite(n) || (typeof args[0] === "string" && args[0].trim() === "")) {
        throw new DogRuntimeError({
          code: "E030",
          title: `Cannot parse '${args[0]}' as number.`,
          line: lineNum,
          why: "Strict conversion requires a valid integer string or number.",
          fix: "Wrap untrusted user text conversions in `try ... catch err ... end`.",
        });
      }
      return Math.trunc(n);
    }
    case "decimal": {
      const n = Number(unwrapNum(args[0])?.n ?? args[0]);
      if (!Number.isFinite(n)) {
        throw new DogRuntimeError({ code: "E030", title: `Cannot parse '${args[0]}' as decimal.`, line: lineNum });
      }
      return makeTypedVal("decimal", n);
    }
    case "len":
      if (typeof args[0] === "string" || Array.isArray(args[0])) return args[0].length;
      if (args[0] && typeof args[0] === "object") return Object.keys(args[0]).length;
      throw new DogRuntimeError({ code: "E021", title: "len() expects text, list, or map.", line: lineNum });
    case "upper":
      return String(args[0]).toUpperCase();
    case "lower":
      return String(args[0]).toLowerCase();
    case "trim":
      return String(args[0]).trim();
    case "title_case":
      return String(args[0]).replace(/\b\w/g, (c) => c.toUpperCase());
    case "contains":
      return String(args[0]).includes(String(args[1]));
    case "replace":
      return String(args[0]).split(String(args[1])).join(String(args[2]));
    case "split":
      return String(args[0]).split(String(args[1]));
    case "join":
      return (args[0] || []).map(formatDogValue).join(String(args[1]));
    case "range": {
      const start = Math.trunc(unwrapNum(args[0])?.n ?? 0);
      const end = Math.trunc(unwrapNum(args[1])?.n ?? 0);
      const out = [];
      for (let k = start; k < end; k++) {
        out.push(k);
        if (out.length > 5000) break;
      }
      return out;
    }
    case "list_sum":
      return (args[0] || []).reduce((acc, v) => acc + (unwrapNum(v)?.n ?? 0), 0);
    case "list_avg": {
      const arr = args[0] || [];
      if (!arr.length) return makeTypedVal("decimal", 0);
      const s = arr.reduce((acc, v) => acc + (unwrapNum(v)?.n ?? 0), 0);
      return makeTypedVal("decimal", s / arr.length);
    }
    case "map_get":
      return args[0]?.[String(args[1])] ?? "";
    case "map_keys":
      return Object.keys(args[0] || {});
    case "abs": {
      const nv = unwrapNum(args[0]);
      return makeTypedVal(nv.isDecimal ? "decimal" : "number", Math.abs(nv.n));
    }
    case "min": {
      const a = unwrapNum(args[0]);
      const b = unwrapNum(args[1]);
      return makeTypedVal(a.isDecimal ? "decimal" : "number", Math.min(a.n, b.n));
    }
    case "max": {
      const a = unwrapNum(args[0]);
      const b = unwrapNum(args[1]);
      return makeTypedVal(a.isDecimal ? "decimal" : "number", Math.max(a.n, b.n));
    }
    case "sqrt": {
      const nv = unwrapNum(args[0]);
      return makeTypedVal("decimal", Math.sqrt(nv.n));
    }
    case "assert":
      if (!args[0]) {
        throw new DogRuntimeError({
          code: "E033",
          title: args[1] ? `Assertion failed: ${args[1]}` : "Assertion failed.",
          line: lineNum,
        });
      }
      return undefined;
    case "fail":
      throw new DogRuntimeError({ code: "E035", title: String(args[0] || "Explicit failure"), line: lineNum });
    case "html_escape":
      return String(args[0] || "")
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
    case "web_nav":
      return `<header class="dg-nav"><strong>${args[0]}</strong></header>`;
    case "web_card":
      return `<article class="dg-card"><h3>${args[0]}</h3><div>${args[1]}</div></article>`;
    case "web_grid":
      return `<div class="dg-grid">${(args[1] || []).join("")}</div>`;
    case "web_button":
      return `<button id="${args[0]}" class="dg-btn">${args[1]}</button>`;
    case "web_input":
      return `<input id="${args[0]}" placeholder="${args[1] || ""}" value="${args[2] || ""}" />`;
    case "html_el":
      return `<${args[0]}>${args[2] || ""}</${args[0]}>`;
    case "dom_mount":
      ctx.output.push(`[dom_mount] ${args[0]} <- ${String(args[1] || "").slice(0, 96)}`);
      return undefined;
    case "dom_set_text":
      ctx.output.push(`[dom_set_text] ${args[0]} = ${formatDogValue(args[1])}`);
      return undefined;
    case "dom_on":
      ctx.output.push(`[dom_on] ${args[0]} (${args[1]}) -> ${args[2]}()`);
      return undefined;
    case "css_bundle":
      return Array.isArray(args[0]) ? args[0].join("\n\n") : ctx.output.filter((l) => l.startsWith("[style]")).join("\n");
    default:
      throw new DogRuntimeError({
        code: "E013",
        title: `Unknown function '${rawName}()' in browser sandbox.`,
        line: lineNum,
        why: "Network/filesystem syscalls run in the native Rust CLI (`dog run`); the browser sandbox executes core language, logic, math, text, collections, and diagnostics.",
        fix: "Install DOG CLI (`dog run`) for full OS, HTTP server, WebSocket, and file APIs.",
      });
  }
}

function splitTopLevelParams(raw) {
  if (!raw.trim()) return [];
  const params = [];
  let depth = 0;
  let cur = "";
  let inStr = false;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (ch === '"') inStr = !inStr;
    if (!inStr) {
      if ("([<".includes(ch)) depth++;
      else if (")]>".includes(ch)) depth = Math.max(0, depth - 1);
      else if (ch === "," && depth === 0) {
        params.push(cur.trim());
        cur = "";
        continue;
      }
    }
    cur += ch;
  }
  if (cur.trim()) params.push(cur.trim());
  return params.map((p) => {
    const eqIdx = p.indexOf("=");
    const left = eqIdx >= 0 ? p.slice(0, eqIdx).trim() : p;
    const defaultExpr = eqIdx >= 0 ? p.slice(eqIdx + 1).trim() : null;
    const colonIdx = left.indexOf(":");
    const name = colonIdx >= 0 ? left.slice(0, colonIdx).trim() : left.trim();
    return { name, defaultExpr };
  });
}

function parseStatements(lines, startIdx = 0, stopTokens = []) {
  const stmts = [];
  let i = startIdx;
  while (i < lines.length) {
    const item = lines[i];
    const trimmed = item.text;
    if (!trimmed) {
      i++;
      continue;
    }
    const lead = trimmed.split(/[^a-zA-Z0-9_]/)[0];
    if (stopTokens.includes(lead) || (lead === "else" && stopTokens.includes("else")) || (lead === "otherwise" && stopTokens.includes("otherwise"))) {
      return { stmts, nextIdx: i, stoppedOn: trimmed, stopLine: item.line };
    }
    if (lead === "end") {
      throw new DogRuntimeError({
        code: "E005",
        title: "Unexpected 'end' without matching opening block.",
        line: item.line,
      });
    }

    if (/^(fn|define|function)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.*)\)\s*(?:(?:->|returns)\s*.+)?$/.test(trimmed)) {
      const m = trimmed.match(/^(?:fn|define|function)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.*)\)\s*(?:(?:->|returns)\s*(.+))?$/);
      const fnName = m[1];
      const params = splitTopLevelParams(m[2]);
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || bodyRes.stoppedOn.split(/[^a-zA-Z0-9_]/)[0] !== "end") {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for function '${fnName}'.`, line: item.line });
      }
      stmts.push({ kind: "fndef", name: fnName, params, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "if" || lead === "unless") {
      const isUnless = lead === "unless";
      const condRaw = trimmed
        .slice(lead.length)
        .replace(/\bthen\s*$/, "")
        .trim();
      const branches = [];
      let curIdx = i + 1;
      let curCond = condRaw;
      let curNegated = isUnless;
      let elseBody = null;

      while (true) {
        const res = parseStatements(lines, curIdx, ["elif", "else", "otherwise", "end"]);
        branches.push({ cond: curCond, negated: curNegated, body: res.stmts, line: lines[curIdx - 1]?.line || item.line });
        if (!res.stoppedOn) {
          throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for '${lead}' block.`, line: item.line });
        }
        const st = res.stoppedOn;
        if (/^end\b/.test(st)) {
          i = res.nextIdx + 1;
          break;
        }
        if (/^(elif|else\s+if|otherwise\s+if)\b/.test(st)) {
          curCond = st
            .replace(/^(elif|else\s+if|otherwise\s+if)\b/, "")
            .replace(/\bthen\s*$/, "")
            .trim();
          curNegated = false;
          curIdx = res.nextIdx + 1;
          continue;
        }
        if (/^(else|otherwise)\b/.test(st)) {
          const elseRes = parseStatements(lines, res.nextIdx + 1, ["end"]);
          if (!elseRes.stoppedOn || !/^end\b/.test(elseRes.stoppedOn)) {
            throw new DogRuntimeError({ code: "E005", title: "Missing 'end' after 'else' branch.", line: res.stopLine });
          }
          elseBody = elseRes.stmts;
          i = elseRes.nextIdx + 1;
          break;
        }
      }
      stmts.push({ kind: "if", branches, elseBody, line: item.line });
      continue;
    }

    if (lead === "while" || lead === "until") {
      const condRaw = trimmed.slice(lead.length).trim();
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for '${lead}' loop.`, line: item.line });
      }
      stmts.push({ kind: lead, cond: condRaw, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "repeat") {
      const countRaw = trimmed
        .slice("repeat".length)
        .replace(/\btimes\s*$/, "")
        .trim();
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'repeat' loop.", line: item.line });
      }
      stmts.push({ kind: "repeat", count: countRaw, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "for") {
      const mFrom = trimmed.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
      const mIn = trimmed.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+(.+)$/);
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'for' loop.", line: item.line });
      }
      if (mFrom) {
        stmts.push({ kind: "for_range", varName: mFrom[1], start: mFrom[2], endExpr: mFrom[3], body: bodyRes.stmts, line: item.line });
      } else if (mIn) {
        stmts.push({ kind: "for_in", varName: mIn[1], iter: mIn[2], body: bodyRes.stmts, line: item.line });
      } else {
        throw new DogRuntimeError({
          code: "E004",
          title: "Invalid 'for' loop syntax.",
          line: item.line,
          fix: "Write `for item in items` or `for i from 1 to 5`.",
        });
      }
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "try") {
      const tryRes = parseStatements(lines, i + 1, ["catch"]);
      if (!tryRes.stoppedOn || !/^catch\b/.test(tryRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Expected 'catch <err>' after 'try'.", line: item.line });
      }
      const errName = tryRes.stoppedOn.replace(/^catch\s+/, "").trim() || "err";
      const catchRes = parseStatements(lines, tryRes.nextIdx + 1, ["end"]);
      if (!catchRes.stoppedOn || !/^end\b/.test(catchRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'try ... catch'.", line: item.line });
      }
      stmts.push({ kind: "try", tryBody: tryRes.stmts, errName, catchBody: catchRes.stmts, line: item.line });
      i = catchRes.nextIdx + 1;
      continue;
    }

    if (lead === "return") {
      const expr = trimmed.slice("return".length).trim();
      stmts.push({ kind: "return", expr: expr || null, line: item.line });
      i++;
      continue;
    }
    if (trimmed === "break") {
      stmts.push({ kind: "break", line: item.line });
      i++;
      continue;
    }
    if (trimmed === "continue") {
      stmts.push({ kind: "continue", line: item.line });
      i++;
      continue;
    }
    if (/^(let|var|const)\s+/.test(trimmed)) {
      const m = trimmed.match(/^(let|var|const)\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*:\s*[^=]+)?\s*=\s*(.+)$/);
      if (!m) {
        throw new DogRuntimeError({
          code: "E004",
          title: "Invalid variable binding.",
          line: item.line,
          fix: "Write `let name = value` or `var count = 0`.",
        });
      }
      stmts.push({ kind: "bind", bindKind: m[1], name: m[2], expr: m[3], line: item.line });
      i++;
      continue;
    }
    if (/^set\s+[a-zA-Z_][a-zA-Z0-9_]*\s+to\s+/.test(trimmed)) {
      const m = trimmed.match(/^set\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+to\s+(.+)$/);
      stmts.push({ kind: "assign", name: m[1], expr: m[2], line: item.line });
      i++;
      continue;
    }
    if (/^(increase|decrease)\s+[a-zA-Z_][a-zA-Z0-9_]*\s+by\s+/.test(trimmed)) {
      const m = trimmed.match(/^(increase|decrease)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+by\s+(.+)$/);
      stmts.push({ kind: "compound", op: m[1] === "increase" ? "+=" : "-=", name: m[2], expr: m[3], line: item.line });
      i++;
      continue;
    }
    if (/^[a-zA-Z_][a-zA-Z0-9_]*\s*(\+=|-=|\*=|\/=|%=|=)\s*/.test(trimmed)) {
      const m = trimmed.match(/^([a-zA-Z_][a-zA-Z0-9_]*)\s*(\+=|-=|\*=|\/=|%=|=)\s*(.+)$/);
      if (m[2] === "=") {
        stmts.push({ kind: "assign", name: m[1], expr: m[3], line: item.line });
      } else {
        stmts.push({ kind: "compound", op: m[2], name: m[1], expr: m[3], line: item.line });
      }
      i++;
      continue;
    }

    if (lead === "style" && (trimmed === "style" || trimmed.startsWith("style "))) {
      const header = trimmed.slice(5).trim();
      if (!header) {
        throw new DogRuntimeError({
          code: "E004",
          title: "Expected selector or 'theme' after 'style'.",
          line: item.line,
          fix: 'Write `style ".card"` or `style theme`.',
        });
      }
      let cursor = i + 1;
      const rules = [];
      let closed = false;
      while (cursor < lines.length) {
        const inner = lines[cursor].text;
        if (!inner) {
          cursor++;
          continue;
        }
        if (inner === "end") {
          closed = true;
          cursor++;
          break;
        }
        if (/^(?:on\s+)?(?:hover|focus|active|mobile|tablet|desktop|dark)$/.test(inner) || /^media\s+"[^"]+"$/.test(inner)) {
          const vLine = lines[cursor].line;
          cursor++;
          let vClosed = false;
          while (cursor < lines.length) {
            const vTxt = lines[cursor].text;
            if (!vTxt) {
              cursor++;
              continue;
            }
            if (vTxt === "end") {
              vClosed = true;
              cursor++;
              break;
            }
            cursor++;
          }
          if (!vClosed) {
            throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for style variant block.", line: vLine });
          }
          continue;
        }
        rules.push(inner);
        cursor++;
      }
      if (!closed) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'style' block.", line: item.line });
      }
      stmts.push({ kind: "style", header, rulesCount: rules.length, line: item.line });
      i = cursor;
      continue;
    }

    if (/^use\s+"/.test(trimmed)) {
      stmts.push({ kind: "use", path: trimmed, line: item.line });
      i++;
      continue;
    }

    stmts.push({ kind: "expr", expr: trimmed, line: item.line });
    i++;
  }
  return { stmts, nextIdx: i, stoppedOn: null, stopLine: null };
}

function execBlock(stmts, ctx) {
  for (const st of stmts) {
    ctx.steps++;
    if (ctx.steps > MAX_STEPS) {
      throw new DogRuntimeError({
        code: "E035",
        title: "Execution step limit reached in browser sandbox.",
        line: st.line,
        why: "Infinite loop protection stopped execution after 8,000 steps.",
        fix: "Ensure your `while` or `until` loop converges.",
      });
    }
    if (st.kind === "use") continue;
    if (st.kind === "style") {
      ctx.astSummary.push(`style ${st.header} (${st.rulesCount} rules)`);
      continue;
    }
    if (st.kind === "fndef") {
      ctx.functions.set(st.name, st);
      ctx.astSummary.push(`fn ${st.name}(${st.params.map((p) => p.name).join(", ")})`);
      continue;
    }
    if (st.kind === "bind") {
      const val = evalExprString(st.expr, ctx, st.line);
      ctx.define(st.name, st.bindKind, val);
      ctx.astSummary.push(`${st.bindKind} ${st.name}: ${typeNameOf(val)}`);
      continue;
    }
    if (st.kind === "assign") {
      const cur = ctx.lookup(st.name);
      if (!cur) {
        throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${st.name}'.`, line: st.line });
      }
      if (cur.kind !== "var") {
        throw new DogRuntimeError({
          code: "E012",
          title: `Cannot reassign immutable binding '${st.name}'.`,
          line: st.line,
          why: `'${st.name}' was declared with '${cur.kind}', not 'var'.`,
          fix: `Change \`${cur.kind} ${st.name} = ...\` to \`var ${st.name} = ...\` if it needs to change.`,
        });
      }
      const val = evalExprString(st.expr, ctx, st.line);
      if (typeNameOf(cur.value) !== typeNameOf(val)) {
        throw new DogRuntimeError({
          code: "E021",
          title: `Type mismatch assigning to '${st.name}'.`,
          line: st.line,
          why: `Expected '${typeNameOf(cur.value)}', got '${typeNameOf(val)}'.`,
        });
      }
      cur.value = val;
      continue;
    }
    if (st.kind === "compound") {
      const cur = ctx.lookup(st.name);
      if (!cur) {
        throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${st.name}'.`, line: st.line });
      }
      if (cur.kind !== "var") {
        throw new DogRuntimeError({
          code: "E012",
          title: `Cannot mutate immutable '${cur.kind}' variable '${st.name}'.`,
          line: st.line,
          fix: `Declare '${st.name}' with \`var\` instead of \`${cur.kind}\`.`,
        });
      }
      const rhs = evalExprString(st.expr, ctx, st.line);
      if (st.op === "+=" && typeof cur.value === "string" && typeof rhs === "string") {
        cur.value += rhs;
        continue;
      }
      const na = unwrapNum(cur.value);
      const nb = unwrapNum(rhs);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: `Type mismatch in '${st.op}' on '${st.name}'.`,
          line: st.line,
        });
      }
      let nextVal = na.n;
      if (st.op === "+=") nextVal += nb.n;
      else if (st.op === "-=") nextVal -= nb.n;
      else if (st.op === "*=") nextVal *= nb.n;
      else if (st.op === "/=") nextVal = na.isDecimal ? nextVal / nb.n : Math.trunc(nextVal / nb.n);
      else if (st.op === "%=") nextVal %= nb.n;
      cur.value = makeTypedVal(na.isDecimal ? "decimal" : "number", nextVal);
      continue;
    }
    if (st.kind === "expr") {
      evalExprString(st.expr, ctx, st.line);
      continue;
    }
    if (st.kind === "return") {
      const value = st.expr ? evalExprString(st.expr, ctx, st.line) : undefined;
      return { type: "return", value };
    }
    if (st.kind === "break") return { type: "break" };
    if (st.kind === "continue") return { type: "continue" };
    if (st.kind === "if") {
      let ran = false;
      for (const b of st.branches) {
        const cond = evalExprString(b.cond, ctx, b.line);
        if (typeNameOf(cond) !== "bool") {
          throw new DogRuntimeError({
            code: "E022",
            title: "Condition must evaluate to 'bool'.",
            line: b.line,
          });
        }
        const pass = b.negated ? !cond : cond;
        if (pass) {
          ran = true;
          ctx.pushScope();
          try {
            const flow = execBlock(b.body, ctx);
            if (flow.type !== "next") return flow;
          } finally {
            ctx.popScope();
          }
          break;
        }
      }
      if (!ran && st.elseBody) {
        ctx.pushScope();
        try {
          const flow = execBlock(st.elseBody, ctx);
          if (flow.type !== "next") return flow;
        } finally {
          ctx.popScope();
        }
      }
      continue;
    }
    if (st.kind === "while" || st.kind === "until") {
      while (true) {
        ctx.steps++;
        if (ctx.steps > MAX_STEPS) {
          throw new DogRuntimeError({ code: "E035", title: "Loop step limit reached.", line: st.line });
        }
        const cond = evalExprString(st.cond, ctx, st.line);
        if (typeNameOf(cond) !== "bool") {
          throw new DogRuntimeError({ code: "E022", title: "Loop condition must be 'bool'.", line: st.line });
        }
        const go = st.kind === "until" ? !cond : cond;
        if (!go) break;
        ctx.pushScope();
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "repeat") {
      const cnt = evalExprString(st.count, ctx, st.line);
      const n = unwrapNum(cnt);
      if (!n || n.isDecimal || n.n < 0) {
        throw new DogRuntimeError({ code: "E025", title: "repeat count must be a non-negative number.", line: st.line });
      }
      for (let k = 0; k < n.n; k++) {
        ctx.pushScope();
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "for_range") {
      const sVal = unwrapNum(evalExprString(st.start, ctx, st.line));
      const eVal = unwrapNum(evalExprString(st.endExpr, ctx, st.line));
      if (!sVal || !eVal || sVal.isDecimal || eVal.isDecimal) {
        throw new DogRuntimeError({ code: "E021", title: "Range bounds must be integers.", line: st.line });
      }
      for (let v = sVal.n; v <= eVal.n; v++) {
        ctx.pushScope();
        ctx.define(st.varName, "let", v);
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "for_in") {
      const list = evalExprString(st.iter, ctx, st.line);
      if (!Array.isArray(list)) {
        throw new DogRuntimeError({ code: "E025", title: "for-in expects a list.", line: st.line });
      }
      for (const item of list) {
        ctx.pushScope();
        ctx.define(st.varName, "let", item);
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "try") {
      try {
        ctx.pushScope();
        const flow = execBlock(st.tryBody, ctx);
        ctx.popScope();
        if (flow.type !== "next") return flow;
      } catch (err) {
        if (ctx.scopes.length > 1) ctx.popScope();
        const msg = err instanceof DogRuntimeError ? err.title : String(err.message || err);
        ctx.pushScope();
        ctx.define(st.errName, "let", msg);
        try {
          const flow = execBlock(st.catchBody, ctx);
          if (flow.type !== "next") return flow;
        } finally {
          ctx.popScope();
        }
      }
    }
  }
  return { type: "next" };
}

export function runDogSandbox(source) {
  const start = Date.now();
  const lines = source.split(/\r?\n/).map((raw, idx) => ({
    line: idx + 1,
    text: stripComment(raw).trim(),
  }));

  const scopes = [new Map()];
  const ctx = {
    scopes,
    functions: new Map(),
    output: [],
    astSummary: [],
    steps: 0,
    pushScope() {
      this.scopes.push(new Map());
    },
    popScope() {
      if (this.scopes.length > 1) this.scopes.pop();
    },
    define(name, kind, value) {
      this.scopes[this.scopes.length - 1].set(name, { kind, value });
    },
    lookup(name) {
      for (let i = this.scopes.length - 1; i >= 0; i--) {
        if (this.scopes[i].has(name)) return this.scopes[i].get(name);
      }
      return null;
    },
  };

  try {
    const parsed = parseStatements(lines, 0, []);
    execBlock(parsed.stmts, ctx);
    return {
      ok: true,
      output: ctx.output,
      astSummary: ctx.astSummary,
      steps: ctx.steps,
      elapsedMs: Date.now() - start,
      error: null,
    };
  } catch (err) {
    const diag =
      err instanceof DogRuntimeError
        ? err
        : new DogRuntimeError({
            code: "E004",
            title: String(err?.message || err),
            line: 1,
          });
    return {
      ok: false,
      output: ctx.output,
      astSummary: ctx.astSummary,
      steps: ctx.steps,
      elapsedMs: Date.now() - start,
      error: diag.format(),
      errorCode: diag.code,
    };
  }
}
