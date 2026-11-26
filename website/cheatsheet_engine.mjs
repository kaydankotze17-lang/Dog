export const CHEATSHEET_CATEGORIES = [
  { id: "all", label: "All Topics" },
  { id: "basics", label: "Basics & Types" },
  { id: "dialects", label: "3 Syntax Styles" },
  { id: "control", label: "Control & Match" },
  { id: "functions", label: "Functions & Lambdas" },
  { id: "stdlib-data", label: "Data, SQLite & Crypto" },
  { id: "concurrency", label: "HTTP & Concurrency" },
  { id: "web-dgss", label: "DOG Web & .dgss" },
  { id: "errors", label: "Errors & Warnings" },
  { id: "cli", label: "CLI & Termux" },
];

export const ENGLISH_VS_CLASSIC_TABLE = [
  { concept: "Immutable / mutable", english: "let name = \"Ada\"\nvar count = 0", classic: "let name: text = \"Ada\"\nvar count: number = 0", simple: "keep name = \"Ada\"\nmake count = 0" },
  { concept: "Reassignment", english: "set count to 10\nincrease count by 2", classic: "count = 10\ncount += 2", simple: "count = 10\nbump count by 2\nadd 2 to count" },
  { concept: "Function header", english: "define add(a: number, b: number = 1) returns number", classic: "fn add(a: number, b: number = 1) -> number", simple: "do add(a, b = 1)\ndo add(a, b = 1) = a + b" },
  { concept: "Branches", english: "if score is at least 90 then ... elif ... else ... end", classic: "if score >= 90 ... elif ... otherwise ... end", simple: "when score >= 90 ... or when ... else ... end" },
  { concept: "Negated condition", english: "unless locked then ... end", classic: "if not locked ... end", simple: "when not locked ... end" },
  { concept: "Loops", english: "repeat 3 times\nuntil done\nfor i from 1 to 5", classic: "for each x in list\nwhile not done\nfor i in range(1, 6)", simple: "loop 3\nloop\neach i in 1..5" },
  { concept: "Comparisons", english: "x is y · x is not y · x is over 5 · x is under 10", classic: "x == y · x != y · x > 5 · x < 10", simple: "x == y · x is y · x > 5 · x < 10" },
  { concept: "Boolean logic", english: "ready and not busy or forced", classic: "ready and not busy or forced (no && / || / !)", simple: "ready and not busy or forced" },
  { concept: "Output / Return", english: "print(msg) / return val", classic: "say(msg) / return val", simple: "show msg / give val" },
  { concept: "Error handling", english: "try ... catch err ... end", classic: "try ... catch err ... end", simple: "try ... oops err ... end" },
  { concept: "Pipe & fallback", english: "let res = raw |> trim |> upper", classic: "let v = map_opt(m, \"k\") ?? \"default\"", simple: "keep v = raw |> trim ?? \"default\"" },
  { concept: "Block closing", english: "end (required on all blocks)", classic: "end (braces reserved for map literals)", simple: "end (or one-liner do f(x) = expr)" },
];

export const DIALECT_COMPARISON_TABLE = ENGLISH_VS_CLASSIC_TABLE;

export const OPERATOR_PRECEDENCE_TABLE = [
  { level: "1 (Highest)", operators: ".   ()   [idx]   [start..end]", description: "Member access, calls, indexing, slices" },
  { level: "2", operators: "-   not", description: "Unary negation and logical inversion" },
  { level: "3", operators: "*   /   %", description: "Multiplication, division, modulo" },
  { level: "4", operators: "+   -", description: "Addition, list/string concatenation, subtraction" },
  { level: "5", operators: "== != < <= > >= is / is not / is at least / is under", description: "Strict type-checked equality and ordering" },
  { level: "6", operators: "and", description: "Short-circuit boolean conjunction" },
  { level: "7", operators: "or", description: "Short-circuit boolean disjunction" },
  { level: "8", operators: "??", description: "Nullish fallback when left value is null" },
  { level: "9 (Lowest)", operators: "|>", description: "Pipeline value into next function or lambda" },
];

export const CHEATSHEET_ENTRIES = [
  {
    id: "variables-types",
    category: "basics",
    title: "Variables, Constants & Type Aliases",
    summary: "Bind immutable constants with let/const, mutable state with var, strict primitive/collection types, and domain type aliases.",
    syntaxBadge: "Core · v1.0",
    playgroundRunnable: true,
    keywords: ["let", "var", "const", "type", "alias", "text", "number", "decimal", "bool", "list", "map", "hex", "binary"],
    code: `type Score = number
const MAX_SCORE = 100
let player: text = "Maya"
var points: Score = 0x10_00 // hex underscores
increase points by 0b1010

print($"Player {player}: {points}/{MAX_SCORE}")`,
  },
  {
    id: "strings-slices-destructuring",
    category: "basics",
    title: "Interpolation, Destructuring & Slices",
    summary: "Explicit $\"...\" interpolation prevents accidental string coercion (E021). Slice lists/text cleanly with [start..end] and unpack with [a, b].",
    syntaxBadge: "Syntax · Slices",
    playgroundRunnable: true,
    keywords: ["slice", "destructuring", "interpolation", "list", "map", "range", "triple"],
    code: `let items = ["alpha", "beta", "gamma", "delta"]
let [first, second] = items
let middle = items[1..3]
let prefix = first[0..3]
let profile = {"lang": "DOG", "version": "1.0"}

print($"first={first}, mid={join(middle, "|")}, pre={prefix}")
print($"map={profile["lang"]} v{profile["version"]}")`,
  },
  {
    id: "structs-records",
    category: "basics",
    title: "Structs & Dotted Field Mutation",
    summary: "Define structured records with typed fields, instantiate with Name { field: value }, and update mutable var struct fields cleanly.",
    syntaxBadge: "Types · Structs",
    playgroundRunnable: true,
    keywords: ["struct", "field", "record", "set", "increase", "object"],
    code: `struct Service
    name: text
    port: number
    healthy: bool
end

var api = Service { name: "dog-gateway", port: 8080, healthy: true }
set api.port to 8443
print($"Service {api.name} on :{api.port} healthy={api.healthy}")`,
  },
  {
    id: "english-vs-classic-run",
    category: "dialects",
    title: "Dual-Syntax Parity (English + Classic)",
    summary: "Mix or choose English phrasing (define/returns/set/increase/is at least) or Classic phrasing (fn/->/=/+=/otherwise) — both compile identically.",
    syntaxBadge: "Parity · Dual",
    playgroundRunnable: true,
    keywords: ["english", "classic", "define", "fn", "returns", "otherwise", "unless", "until", "repeat"],
    code: `define classify(score: number) returns text
    if score is at least 90 then
        return "A"
    elif score is over 74 then
        return "B"
    else
        return "C"
    end
end

fn clamp_zero(n: number) -> number
    if n < 0
        return 0
    otherwise
        return n
    end
end

print($"Grade={classify(92)}, Clamped={clamp_zero(-5)}")`,
  },
  {
    id: "simple-style-basics",
    category: "dialects",
    title: "Simple Style (Ultra-Short Syntax)",
    summary: "Write DOG faster with zero type noise: keep (const), make (var), do name() = expr, give (return), show (print), when / or when, loop N, each x in a..b, add / take / bump, and try / oops.",
    syntaxBadge: "Simple · New",
    playgroundRunnable: true,
    keywords: ["simple", "keep", "make", "do", "give", "show", "when", "or when", "loop", "each", "stop", "skip", "oops", "bump", "add", "take", "check"],
    code: `do double(n) = n * 2

do when_ok(s)
    when s >= 15
        give "Ready"
    else
        give "Pending"
    end
end

make score = 10
bump score by 5
keep label = when_ok(score)

make sum = 0
each i in 1..5
    when i == 3
        skip
    end
    add i to sum
end

check sum == 12
show $"label={label} sum={sum} double={double(7)}"`,
  },
  {
    id: "enums-match-guards",
    category: "control",
    title: "Enums & Pattern Matching with Guards",
    summary: "Model algebraic variants (unit and tuple payloads) with enum, then branch exhaustively with match, destructuring payloads, comma patterns, and if guards.",
    syntaxBadge: "Control · Match",
    playgroundRunnable: true,
    keywords: ["enum", "match", "case", "guard", "pattern", "variant", "when", "else"],
    code: `enum Result
    Ok(number)
    Err(text)
    Empty
end

fn describe(res: Result) -> text
    match res
        case Result.Ok(n) if n > 100
            return $"Large ok: {n}"
        case Result.Ok(n)
            return $"Ok: {n}"
        case Result.Err(msg)
            return $"Error: {msg}"
        else
            return "Empty"
    end
end

print(describe(Result.Ok(250)))
print(describe(Result.Err("timeout")))
print(describe(Result.Empty))`,
  },
  {
    id: "loops-try-catch",
    category: "control",
    title: "Loops, Ranges & Try/Catch Recovery",
    summary: "Iterate cleanly using repeat N times, for i from a to b, for each item in list, until, while, and recover from runtime errors with try/catch.",
    syntaxBadge: "Safety · Loops",
    playgroundRunnable: true,
    keywords: ["for", "repeat", "until", "while", "try", "catch", "fail", "assert"],
    code: `var total = 0
for step from 1 to 4
    increase total by step
end

try
    let bad = number("not-a-num")
    print(bad)
catch err
    print($"Recovered ({total}): {err}")
end`,
  },
  {
    id: "lambdas-closures-combinators",
    category: "functions",
    title: "Lambdas, Lexical Closures & Combinators",
    summary: "Pass inline arrow lambdas fn(x) => expr, multi-line fn blocks, first-class functions, or captures into map, filter, reduce, list_find, list_any, and list_all.",
    syntaxBadge: "FP · Closures",
    playgroundRunnable: true,
    keywords: ["lambda", "fn", "closure", "map", "filter", "reduce", "list_find", "list_any", "list_all"],
    code: `fn make_adder(base: number) -> fn
    return fn(x) => x + base
end

let add10 = make_adder(10)
let nums = [5, 12, 18, 25]
let above10 = filter(nums, fn(x) => x > 10)
let shifted = map(above10, add10)
let total = reduce(shifted, 0, fn(acc, x) => acc + x)

print($"shifted={join(shifted, ",")} sum={total}")`,
  },
  {
    id: "pipe-nullish-helpers",
    category: "functions",
    title: "Pipes (|>), Nullish (??) & Modern Helpers",
    summary: "Chain transformations cleanly left-to-right with |>, supply defaults on null with ??, and validate data with regex_match, list_chunk, schema_validate, and semver_compare.",
    syntaxBadge: "Pipe · Helpers",
    playgroundRunnable: true,
    keywords: ["pipe", "|>", "nullish", "??", "null", "map_opt", "regex_match", "list_chunk", "list_zip", "schema_validate", "semver_compare"],
    code: `keep clean = "  simple dog v1.0  " |> trim |> upper
keep cfg = {"host": "127.0.0.1"}
keep port = map_opt(cfg, "port") ?? "8080"

keep chunks = [1, 2, 3, 4] |> list_chunk(2)
keep valid = regex_match("^SIMPLE", clean)
keep cmp = semver_compare("1.0.0", "0.3.0")

show $"clean={clean} port={port} chunks={len(chunks)} match={valid} cmp={cmp}"`,
  },
  {
    id: "sqlite-transactions",
    category: "stdlib-data",
    title: "Relational SQLite, Prepared Params & Query Builder",
    summary: "Open in-memory or named SQLite stores with db_open, bind ? parameters safely in db_exec/db_query, wrap atomic transactions with db_begin/commit/rollback, and compose query_builder.",
    syntaxBadge: "std.data · SQL",
    playgroundRunnable: true,
    keywords: ["sqlite", "db_open", "db_exec", "db_query", "db_begin", "db_commit", "db_rollback", "query_builder"],
    code: `let db = db_open(":memory:")
db_exec(db, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, role TEXT)")
db_exec(db, "INSERT INTO users (name, role) VALUES (?, ?)", ["Ada", "admin"])
db_begin(db)
db_exec(db, "INSERT INTO users (name, role) VALUES (?, ?)", ["Temp", "guest"])
db_rollback(db)

let admins = db_query(db, "SELECT id, name FROM users WHERE role = ?", ["admin"])
print($"Admins count={len(admins)} json={json_stringify(admins)}")`,
  },
  {
    id: "crypto-auth-ed25519",
    category: "stdlib-data",
    title: "SHA-512, Password Hashing, AES-256 & Ed25519",
    summary: "Zero-dependency cryptographic suite: SHA-256/SHA-512 digests, constant-time PBKDF2 password hashing, authenticated AES-256-CTR-HMAC encryption, and RFC 8032 Ed25519 signatures.",
    syntaxBadge: "std.crypto",
    playgroundRunnable: true,
    keywords: ["sha256", "sha512", "password_hash", "password_verify", "encrypt_aes256", "decrypt_aes256", "ed25519", "hmac"],
    code: `let digest = sha512("abc")
let pw = password_hash("correct-horse-battery")
let pw_ok = password_verify("correct-horse-battery", pw)

let cipher = encrypt_aes256("master-key", "secret payload")
let plain = decrypt_aes256("master-key", cipher)

let kp = ed25519_keypair("demo-seed")
let sig = ed25519_sign(kp["private_key"], "release:v1.0.0")
let sig_ok = ed25519_verify(kp["public_key"], "release:v1.0.0", sig)

print($"sha512={digest[0..12]} pw={pw_ok} aes={plain} ed={sig_ok}")`,
  },
  {
    id: "http-concurrency-tasks",
    category: "concurrency",
    title: "HTTP Builders, Tasks, Channels & Atomics",
    summary: "Construct typed HTTP requests, route groups, rate limits, and OpenAPI specs alongside spawn_task, await_all, channels, and atomic counters.",
    syntaxBadge: "std.net · Sync",
    playgroundRunnable: true,
    keywords: ["spawn_task", "await_task", "await_all", "channel_create", "channel_send", "channel_recv", "atomic", "http_req", "openapi_spec"],
    code: `let jobs = [spawn_task(fn(n) => n * n, 6), spawn_task(fn(n) => n * n, 9)]
let squares = await_all(jobs)
let ch = channel_create()
channel_send(ch, "ready")
let atom = atomic_create(10)
let next = atomic_add(atom, 5)
let spec = openapi_spec("DOG API", "1.0.0", ["/health", "/users/:id"])

print($"squares={join(squares, ",")} msg={channel_recv(ch)} atom={next} ok={contains(spec, "paths")}")`,
  },
  {
    id: "dgss-inheritance-states",
    category: "web-dgss",
    title: "DOG CSS (.dgss): Shorthands, Extend & Media Queries",
    summary: "Write crisp stylesheets with variables (vars ... end), extend: inheritance, semantic shorthands (bg, ink, pad, px, py, radius, shadow, cols), pseudo states, and responsive breakpoints.",
    syntaxBadge: ".dgss · Style",
    playgroundRunnable: false,
    keywords: ["dgss", "vars", "rule", "extend", "bg", "ink", "pad", "on hover", "on mobile", "animate", "tree-shake"],
    code: `vars
    surface: #FFFFFF
    accent: #C85A11
end

rule .dg-card
    bg: $surface
    pad: 18px
    radius: 10px
    shadow: sm
end

rule .dg-card-accent
    extend: .dg-card
    border: 1px solid $accent
    on hover
        shadow: md
    end
end

on mobile
    rule .dg-card
        pad: 12px
    end
end`,
  },
  {
    id: "std-web-reactive",
    category: "web-dgss",
    title: "std.web: Components, Reactive State & Delegation",
    summary: "Build XSS-escaped UI components with web_component, web_tabs, web_form, reactive state watchers (web_state_set / web_on_state), and dom_delegate.",
    syntaxBadge: "std.web · DOM",
    playgroundRunnable: false,
    keywords: ["std.web", "web_component", "web_render_component", "web_state_set", "web_on_state", "dom_delegate", "web_tabs", "dom_mount"],
    code: `use "std.web"

define render_badge(props: map) returns text
    return web_badge(props["label"], "success")
end

web_component("status-badge", "render_badge")
web_on_state("tab", "on_tab_change")
dom_mount("#app", web_tabs("nav-tabs", {"home": "Home", "docs": "Docs"}, "home"))
dom_delegate("#nav-tabs", ".dg-tab-btn", "click", "on_tab_click")`,
  },
  {
    id: "diagnostics-codes",
    category: "errors",
    title: "Compiler Errors (E001–E041) & Warnings (W001–W003)",
    summary: "Every diagnostic explains What happened, WHY, and FIX, plus --diagnostics=json metadata and optimization reports.",
    syntaxBadge: "Diagnostics",
    playgroundRunnable: true,
    keywords: ["E001", "E005", "E012", "E021", "E026", "E034", "W001", "W002", "W003", "warning", "error"],
    code: `# W001: unreachable statement after return
# W002: constant fold / redundant branch notice
# W003: unused local variable (prefix _ to silence)
define folded_math() returns number
    let _ignored = 123
    return 6 * 7
end

print($"Folded result={folded_math()}")`,
  },
  {
    id: "cli-termux-commands",
    category: "cli",
    title: "CLI Toolchain & Zero-Error Termux Updater",
    summary: "Run scripts, verify bytecode bundles, compile standalone websites, debug line-by-line, open the mobile Studio IDE, and update cleanly on Termux.",
    syntaxBadge: "CLI · Termux",
    playgroundRunnable: false,
    keywords: ["cli", "termux", "update", "dog run", "dog web", "dog studio", "dog debug", "dog check", "curl"],
    code: `# Run, type-check, and compile websites
dog check app.dg --diagnostics=json
dog run app.dg
dog web build app.dg --minify --tree-shake --out dist_web
dog debug app.dg
dog studio .

# Termux zero-error installer & updater (bypasses stale APT mirrors)
curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
dog update --check`,
  },
];

export function filterCheatSheetEntries(entries = CHEATSHEET_ENTRIES, { category = "all", q = "" } = {}) {
  const cat = String(category || "all").trim().toLowerCase();
  const query = String(q || "").trim().toLowerCase();

  return entries.filter((entry) => {
    if (cat !== "all" && entry.category !== cat) return false;
    if (!query) return true;
    const haystack = [
      entry.title,
      entry.summary,
      entry.syntaxBadge || "",
      entry.code,
      ...(entry.keywords || []),
    ]
      .join("\n")
      .toLowerCase();
    return haystack.includes(query);
  });
}

export function formatCheatSheetMarkdown(
  entries = CHEATSHEET_ENTRIES,
  dialectRows = ENGLISH_VS_CLASSIC_TABLE,
  precedenceRows = OPERATOR_PRECEDENCE_TABLE
) {
  const lines = [
    "# DOG Programming Language Cheat Sheet (v1.0.0)",
    "",
    "Official single-page syntax, 3-dialect (English, Classic, Simple), standard library, `.dgss` stylesheet, diagnostics, and CLI reference.",
    "",
    "## English Style vs Classic Style vs Simple Style",
    "",
    "| Concept | English Style | Classic Style | Simple Style |",
    "| :--- | :--- | :--- | :--- |",
  ];

  for (const row of dialectRows) {
    const eng = row.english.replace(/\n/g, " / ").replace(/\|/g, "\\|");
    const cls = row.classic.replace(/\n/g, " / ").replace(/\|/g, "\\|");
    const smp = (row.simple || "").replace(/\n/g, " / ").replace(/\|/g, "\\|");
    lines.push(`| ${row.concept} | \`${eng}\` | \`${cls}\` | \`${smp}\` |`);
  }

  lines.push("", "## Operator Precedence", "", "| Level | Operators | Meaning |", "| :--- | :--- | :--- |");
  for (const op of precedenceRows) {
    lines.push(`| ${op.level} | \`${op.operators}\` | ${op.description} |`);
  }

  for (const cat of CHEATSHEET_CATEGORIES.filter((c) => c.id !== "all")) {
    const group = entries.filter((e) => e.category === cat.id);
    if (group.length === 0) continue;
    lines.push("", `## ${cat.label}`, "");
    for (const item of group) {
      lines.push(`### ${item.title}`);
      lines.push("");
      lines.push(item.summary);
      lines.push("");
      lines.push("```dog");
      lines.push(item.code);
      lines.push("```");
      lines.push("");
    }
  }

  return lines.join("\n");
}
