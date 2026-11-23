export const STD_MODULES = [
  {
    id: "std.io",
    title: "std.io",
    summary: "Interactive keyboard prompts, yes/no confirmation, choice menus, hidden passwords, tables, and terminal output.",
    example: `use "std.io"\n\nlet name = std.io.ask("Your name? ")\nif std.io.confirm("Launch server?") then\n    std.io.print($"Starting workspace for {name}")\nend`,
    functions: [
      { name: "print(value)", sig: "any -> void", desc: "Print a value followed by a newline (alias for say)." },
      { name: "say(value)", sig: "any -> void", desc: "Print any non-void value followed by a newline." },
      { name: "ask(prompt?)", sig: "text -> text", desc: "Display inline prompt and read a line from keyboard input." },
      { name: "confirm(prompt)", sig: "text -> bool", desc: "Interactive (y/n) confirmation returning true or false." },
      { name: "choose(prompt, options)", sig: "(text, list<text>) -> text", desc: "Numbered selection menu returning chosen item." },
      { name: "password(prompt?)", sig: "text -> text", desc: "Read sensitive input with terminal echo suppressed." },
      { name: "read_key()", sig: "() -> text", desc: "Read single keypress (Enter, Esc, Up, Down, Left, Right, Space, char)." },
      { name: "progress(label, cur, total)", sig: "(text, number, number) -> void", desc: "Render a live progress bar in the terminal." },
      { name: "table(headers, rows)", sig: "(list<text>, list<list<text>>) -> void", desc: "Print an aligned ASCII-bordered table." },
      { name: "clear()", sig: "() -> void | (var list<T>) -> void", desc: "Clear terminal screen (0 args) or empty a mutable list (1 arg)." },
    ],
  },
  {
    id: "std.net",
    title: "std.net",
    summary: "Built-in HTTP/HTTPS client & server, dynamic routing, RFC 6455 WebSockets, UDP, raw TCP sockets, DNS, and cookies.",
    example: `use "std.net"\n\nlet srv = std.net.http_listen("127.0.0.1:8080")\nlet req = std.net.http_accept(srv)\nstd.net.http_reply(req, 200, "text/html", "<h1>Hello from DOG</h1>")`,
    functions: [
      { name: "http_listen(addr)", sig: "text -> number", desc: "Bind a local HTTP server socket and return server ID." },
      { name: "http_accept(srv)", sig: "number -> map<text, text>", desc: "Wait for next HTTP request (method, path, query, body, headers)." },
      { name: "http_reply(req, status, ctype, body)", sig: "(map, number, text, text) -> void", desc: "Send complete HTTP response back to the client." },
      { name: "http_route_match(pattern, path)", sig: "(text, text) -> map<text, text>", desc: "Match dynamic routes like /users/:id/*rest." },
      { name: "http_get(url)", sig: "text -> text", desc: "Perform HTTP/HTTPS GET with redirect support." },
      { name: "http_post(url, body, ctype)", sig: "(text, text, text) -> text", desc: "Send HTTP/HTTPS POST payload." },
      { name: "ws_upgrade(req)", sig: "map<text, text> -> number", desc: "Upgrade incoming HTTP request to RFC 6455 WebSocket." },
      { name: "ws_connect(url)", sig: "text -> number", desc: "Connect as WebSocket client to ws:// endpoint." },
      { name: "ws_send(id, msg)", sig: "(number, text) -> void", desc: "Send text frame over WebSocket connection." },
      { name: "udp_bind(addr)", sig: "text -> number", desc: "Bind a UDP datagram socket for peer messaging." },
      { name: "dns_lookup(host)", sig: "text -> list<text>", desc: "Resolve hostnames to IP addresses." },
      { name: "local_ip()", sig: "() -> text", desc: "Detect primary local LAN IPv4 address." },
    ],
  },
  {
    id: "std.text",
    title: "std.text",
    summary: "UTF-8 string utilities, case conversion, trimming, glob matching, HTML escaping, and explicit type casting.",
    example: `use "std.text"\n\nlet clean = std.text.trim("  dog lang  ")\nstd.io.print(std.text.title_case(clean))`,
    functions: [
      { name: "text(val)", sig: "any -> text", desc: "Explicit string conversion for any non-void type." },
      { name: "number(val)", sig: "text | number | decimal -> number", desc: "Strict 64-bit integer conversion." },
      { name: "decimal(val)", sig: "text | number | decimal -> decimal", desc: "Strict 64-bit float conversion." },
      { name: "upper(s) / lower(s)", sig: "text -> text", desc: "Convert text to uppercase or lowercase." },
      { name: "title_case(s)", sig: "text -> text", desc: "Capitalize each word boundary in a string." },
      { name: "trim(s)", sig: "text -> text", desc: "Strip leading and trailing whitespace." },
      { name: "split(s, sep)", sig: "(text, text) -> list<text>", desc: "Split text into parts by separator." },
      { name: "join(list, sep)", sig: "(list<text>, text) -> text", desc: "Join text list using separator." },
      { name: "glob_match(pat, s)", sig: "(text, text) -> bool", desc: "Match wildcard patterns (* and ?)." },
      { name: "html_escape(s)", sig: "text -> text", desc: "Escape HTML characters safely for web templates." },
    ],
  },
  {
    id: "std.math",
    title: "std.math",
    summary: "Integer and floating-point math, trigonometry, logarithms, clamping, GCD/LCM, constants, and random integers.",
    example: `use "std.math"\n\nlet hyp = std.math.sqrt(std.math.pow(3.0, 2.0) + std.math.pow(4.0, 2.0))\nstd.io.print($"Hypotenuse = {hyp}")`,
    functions: [
      { name: "abs(n) / sign(n)", sig: "number | decimal -> number | decimal", desc: "Absolute value or numeric sign (-1, 0, 1)." },
      { name: "min(a, b) / max(a, b)", sig: "(T, T) -> T", desc: "Minimum or maximum of two matching numbers." },
      { name: "clamp(x, lo, hi)", sig: "(T, T, T) -> T", desc: "Constrain value cleanly within [lo, hi]." },
      { name: "sqrt(x) / pow(b, e)", sig: "(decimal, ...) -> decimal", desc: "Square root and exponentiation." },
      { name: "sin(r) / cos(r) / tan(r)", sig: "decimal -> decimal", desc: "Trigonometric functions in radians." },
      { name: "log(x) / log10(x) / exp(x)", sig: "decimal -> decimal", desc: "Natural log, base-10 log, and e^x." },
      { name: "gcd(a, b) / lcm(a, b)", sig: "(number, number) -> number", desc: "Greatest common divisor and least common multiple." },
      { name: "rand_int(min, max)", sig: "(number, number) -> number", desc: "Cryptographically seeded random integer in [min, max]." },
    ],
  },
  {
    id: "std.collections",
    title: "std.collections",
    summary: "Strictly homogeneous lists, in-place var mutations, sorting, deduplication, reductions, and typed maps.",
    example: `var scores: list<number> = [92, 74, 88]\nsort(scores)\nprint($"Average score: {list_avg(scores)}")`,
    functions: [
      { name: "push(var list, item)", sig: "(var list<T>, T) -> void", desc: "Append item to mutable list." },
      { name: "pop(var list)", sig: "var list<T> -> T", desc: "Remove and return final element." },
      { name: "sort(var list) / reverse(var list)", sig: "var list<T> -> void", desc: "Sort or reverse mutable list in-place." },
      { name: "list_unique(items)", sig: "list<T> -> list<T>", desc: "Return list preserving first occurrences only." },
      { name: "list_sum(items) / list_avg(items)", sig: "list<T> -> T | decimal", desc: "Sum or arithmetic average of numeric lists." },
      { name: "range(start, end)", sig: "(number, number) -> list<number>", desc: "Generate integer range [start, end)." },
      { name: "map_get(m, key) / map_get_or(m, key, def)", sig: "map<K, V> -> V", desc: "Look up dictionary keys safely." },
      { name: "map_set(var m, key, val)", sig: "(var map<K, V>, K, V) -> void", desc: "Insert or update key in mutable map." },
    ],
  },
  {
    id: "std.files",
    title: "std.files",
    summary: "File reading/writing, recursive directory listing, path joining, metadata checks, and copy/move operations.",
    example: `use "std.files"\n\nlet out = std.files.path_join(std.files.cwd(), "notes.txt")\nstd.files.write(out, "Saved cleanly from DOG\\n")`,
    functions: [
      { name: "file_read(path) / write(path, s)", sig: "(text, ...) -> text | void", desc: "Read or write UTF-8 file contents." },
      { name: "file_append(path, s)", sig: "(text, text) -> void", desc: "Append text cleanly to existing or new file." },
      { name: "file_exists(path) / is_file(p) / is_dir(p)", sig: "text -> bool", desc: "Check filesystem existence and entry kind." },
      { name: "dir_list(path)", sig: "text -> list<text>", desc: "List files and directories inside a folder." },
      { name: "path_join(a, b)", sig: "(text, text) -> text", desc: "Cross-platform path joining." },
      { name: "path_dirname(p) / path_basename(p) / path_ext(p)", sig: "text -> text", desc: "Extract parent dir, filename, or extension." },
      { name: "mkdir(p) / rmdir(p) / cwd() / temp_dir()", sig: "text -> void | text", desc: "Create/remove directories and resolve standard paths." },
    ],
  },
  {
    id: "std.data",
    title: "std.data",
    summary: "Zero-dependency JSON, TOML, CSV parsing & formatting, plus embedded persistent JSON KV database helpers.",
    example: `use "std.data"\n\nlet cfg = std.data.toml_parse("[server]\\nport = 8080")\nstd.data.kv_set(".state.json", "port", map_get(cfg, "server.port"))`,
    functions: [
      { name: "json_parse(raw) / json_read(path)", sig: "text -> map<text, text>", desc: "Parse JSON objects directly from strings or files." },
      { name: "json_stringify(val) / json_pretty(val)", sig: "any -> text", desc: "Serialize DOG maps, lists, or structs to JSON." },
      { name: "json_write(path, val)", sig: "(text, any) -> void", desc: "Pretty-write DOG data structures directly to disk." },
      { name: "toml_parse(raw)", sig: "text -> map<text, text>", desc: "Parse flattened TOML sections and key/value pairs." },
      { name: "csv_parse(raw) / csv_stringify(rows)", sig: "text <-> list<list<text>>", desc: "RFC 4180 CSV parser and serializer." },
      { name: "kv_set(db, k, v) / kv_get(db, k) / kv_keys(db)", sig: "(text, ...) -> ...", desc: "Persistent local JSON-backed key-value storage." },
    ],
  },
  {
    id: "std.crypto",
    title: "std.crypto",
    summary: "Pure-Rust SHA-1, SHA-256, SHA-512, HMAC-SHA256 verification, Hex, Base64/Base64URL, and CSPRNG tokens.",
    example: `use "std.crypto"\n\nlet sig = std.crypto.hmac_sha256("secret-key", "payload")\nassert(std.crypto.hmac_sha256_verify("secret-key", "payload", sig))`,
    functions: [
      { name: "sha256(s) / sha512(s) / sha1(s)", sig: "text -> text", desc: "Hex-encoded cryptographic digests implemented in pure std Rust." },
      { name: "hmac_sha256(key, msg)", sig: "(text, text) -> text", desc: "Keyed HMAC-SHA256 message authentication code." },
      { name: "hmac_sha256_verify(key, msg, sig)", sig: "(text, text, text) -> bool", desc: "Constant-time HMAC signature verification." },
      { name: "base64_encode(s) / base64_decode(s)", sig: "text -> text", desc: "Standard Base64 and Base64URL encoding/decoding." },
      { name: "hex_encode(s) / hex_decode(h)", sig: "text -> text", desc: "Lowercase hex encoding and validated decoding." },
      { name: "rand_bytes_hex(count)", sig: "number -> text", desc: "Generate cryptographically random hex bytes." },
    ],
  },
  {
    id: "std.system",
    title: "std.system",
    summary: "CLI arguments, .env loading, subprocess spawning, OS/arch inspection, UTC date formatting, and UUIDv4 generation.",
    example: `use "std.system"\n\nlet loaded = std.system.env_load(".env")\nprint($"Running on {std.system.os_name()}/{std.system.os_arch()} at {std.system.time_utc()}")`,
    functions: [
      { name: "args()", sig: "() -> list<text>", desc: "Retrieve CLI arguments passed after `--`." },
      { name: "env_get(k) / env_set(k, v) / env_load(path)", sig: "text -> ...", desc: "Read/write environment variables and parse .env files." },
      { name: "exec_cmd(cmd)", sig: "text -> map<text, text>", desc: "Execute shell command returning code, stdout, and stderr." },
      { name: "time_now_ms() / time_utc() / time_format(ms, fmt)", sig: "... -> number | text", desc: "Millisecond epoch timestamps and strftime-style UTC formatting." },
      { name: "os_name() / os_arch() / uuid_v4()", sig: "() -> text", desc: "Host platform inspection and RFC 4122 v4 UUIDs." },
      { name: "sleep_ms(ms)", sig: "number -> void", desc: "Pause execution cleanly for N milliseconds." },
    ],
  },
  {
    id: "std.cli",
    title: "std.cli",
    summary: "Configurable terminal messaging, progress indicators, spinners, color capability detection, and theme control.",
    example: `use "std.cli"\n\nstd.cli.theme("kali")\nstd.cli.prefix("api")\nstd.cli.success("Bound local endpoint without external crates")`,
    functions: [
      { name: "cli_info(m) / success(m) / warn(m) / error(m) / debug(m)", sig: "text -> void", desc: "Themed, timestamped structured CLI log lines." },
      { name: "cli_theme(name)", sig: "text -> void", desc: "Switch active terminal theme (default, kali, dracula, solar, mono)." },
      { name: "cli_progress(label, cur, tot)", sig: "(text, number, number) -> void", desc: "Render adaptive progress bar with TTY/ASCII fallback." },
      { name: "cli_spinner(step, label)", sig: "(number, text) -> void", desc: "Render frame of animated terminal spinner." },
      { name: "cli_supports_color() / cli_is_tty()", sig: "() -> bool", desc: "Inspect live terminal ANSI and TTY capabilities." },
    ],
  },
  {
    id: "std.css",
    title: "std.css",
    summary: "DOG Style Sheets (.dgss DSL) with variables, shorthands (`bg`, `ink`, `pad`, `cols`), hover states, keyframes, and media queries.",
    example: `use "std.css"\n\nlet css = std.css.compile("vars\\n  accent: #C85A11\\nend\\nrule .card\\n  bg: #FFFFFF\\n  pad: 20px\\n  radius: 10px\\nend")`,
    functions: [
      { name: "dog_css(dsl)", sig: "text -> text", desc: "Compile a `.dgss` DOG Style Sheet into standard browser CSS." },
      { name: "css_rule(selector, props)", sig: "(text, map<text,text>) -> text", desc: "Emit a CSS rule block from typed DOG CSS shorthands." },
      { name: "css_vars(vars)", sig: "map<text,text> -> text", desc: "Generate `:root` CSS custom properties from a map." },
      { name: "css_media(query, css)", sig: "(text, text) -> text", desc: "Wrap rules inside `@media` (`mobile`, `tablet`, `desktop`, `dark`)." },
      { name: "css_keyframes(name, steps)", sig: "(text, map<text,text>) -> text", desc: "Build named `@keyframes` animation stops." },
      { name: "css_theme(bg, surface, ink, accent, font)", sig: "(text x5) -> text", desc: "Emit a clean responsive base theme for `.dg-*` components." },
      { name: "css_bundle(parts)", sig: "list<text> -> text", desc: "Combine multiple compiled CSS blocks into one stylesheet." },
    ],
  },
  {
    id: "std.web",
    title: "std.web",
    summary: "Client-side interactive DOM APIs (`dom_on`, `dom_set_text`, `dom_toggle_class`), localStorage persistence, routing, and safe HTML builders.",
    example: `use "std.web"\n\ndefine on_hit() returns void\n    dom_set_text("#status", "Clicked from DOG!")\nend\n\ndom_mount("#app", web_card("App", web_button("go", "Run")))\ndom_on("#go", "click", "on_hit")`,
    functions: [
      { name: "dom_mount(selector, html)", sig: "(text, text) -> void", desc: "Mount compiled HTML UI into `#app` or any DOM selector." },
      { name: "dom_on(selector, event, fn_name)", sig: "(text, text, text) -> void", desc: "Attach a live browser event listener (`click`, `input`, etc.) to a DOG function." },
      { name: "dom_set_text(sel, val) / dom_set_html(sel, html)", sig: "(text, text) -> void", desc: "Update element textContent or innerHTML interactively." },
      { name: "dom_get_value(sel) / dom_set_value(sel, val)", sig: "(text, ...) -> text | void", desc: "Read or update input/textarea values directly from DOG." },
      { name: "dom_add_class / remove_class / toggle_class", sig: "(text, text) -> void | bool", desc: "Manipulate CSS classes dynamically in response to user actions." },
      { name: "dom_set_style(sel, prop, val)", sig: "(text, text, text) -> void", desc: "Apply inline CSS properties cleanly." },
      { name: "web_nav / web_card / web_grid / web_button / web_input", sig: "... -> text", desc: "Composable, XSS-escaped UI component builders." },
      { name: "web_storage_set(k, v) / web_storage_get(k)", sig: "(text, ...) -> void | text", desc: "Read and persist browser `localStorage` state." },
    ],
  },
];

export const SYNTAX_SHOWCASE = {
  english: {
    label: "English Basics",
    file: "examples/english_basics.dg",
    notes: [
      "Declare functions with `define ... returns ...` or `fn ... -> ...`, including optional default parameter values.",
      "Compare values using plain English phrases (`is`, `is not`, `is at least`, `is less than`, `is over`) alongside symbolic operators.",
      "Contextual connector words (`then`, `times`, `from`, `to`) never reserve common variable names.",
    ],
    englishCode: `# Easy-to-read English-style DOG program
define greet(name: text, title: text = "Friend") returns text
    return $"Hello, {title} {name}!"
end

var points = 0
let badge = "Starter"
print(greet("Alex"))

if badge is "Starter" then
    increase points by 10
elif badge is "Pro" then
    increase points by 25
else
    set points to 5
end

unless points is less than 10 then
    print($"Nice! {points} pts (rank: {badge})")
end

for step from 1 to 3
    print($"Countdown step {step}")
end`,
    classicCode: `# Equivalent classic DOG syntax (same typed AST)
fn greet(name: text, title: text = "Friend") -> text
    return $"Hello, {title} {name}!"
end

var points = 0
let badge = "Starter"
say(greet("Alex"))

if badge == "Starter"
    points += 10
otherwise if badge == "Pro"
    points += 25
otherwise
    points = 5
end

if not (points < 10)
    say($"Nice! {points} pts (rank: {badge})")
end

for step in range(1, 4)
    say($"Countdown step {step}")
end`,
  },
  web: {
    label: "Local Web Server",
    file: "examples/web_server.dg",
    notes: [
      "Zero external frameworks: `http_listen()` binds directly to any local or LAN interface (`0.0.0.0:8080`).",
      "Dynamic path routing (`http_route_match(\"/hello/:name\", path)`) and persistent local KV storage (`kv_set` / `kv_get`).",
      "Runs identically on Linux laptops, servers, and Android Termux phones.",
    ],
    englishCode: `let srv = http_listen("127.0.0.1:8080")
cli_info($"Serving on http://127.0.0.1:{http_port(srv)}")

while true
    let req = http_accept(srv)
    let path = map_get(req, "path")

    if path is "/" then
        let visits = 1
        http_reply(req, 200, "text/html", $"<h1>DOG Server · Visit #{visits}</h1>")
    elif path is "/api/status" then
        http_reply(req, 200, "application/json", json_stringify({"ok": "true", "os": os_name()}))
    else
        http_reply(req, 404, "text/plain", "Not Found")
    end
end`,
    classicCode: `let srv = http_listen("127.0.0.1:8080")
cli_info($"Serving on http://127.0.0.1:{http_port(srv)}")

while true
    let req = http_accept(srv)
    let path = map_get(req, "path")

    if path == "/"
        let visits = 1
        http_reply(req, 200, "text/html", $"<h1>DOG Server · Visit #{visits}</h1>")
    otherwise if path == "/api/status"
        http_reply(req, 200, "application/json", json_stringify({"ok": "true", "os": os_name()}))
    otherwise
        http_reply(req, 404, "text/plain", "Not Found")
    end
end`,
  },
  bot: {
    label: "Discord Bot",
    file: "examples/discord_bot.dg",
    notes: [
      "Host Discord webhook and REST v10 command bridges locally with `http_listen` and `http_request`.",
      "Supports `/interactions` Type-1 PING acks, environment config via `env_get` / `env_load`, and `try ... catch err` resilience.",
      "Backed by pure Rust SHA-256 / HMAC-SHA256 verification utilities.",
    ],
    englishCode: `define handle_command(raw: text) returns text
    let cmd = lower(trim(raw))
    if cmd is "!ping" then
        return $"Pong! Alive on {os_name()}/{os_arch()}"
    elif cmd is "!roll" then
        return $"Rolled a {rand_int(1, 20)} (d20)"
    else
        return $"Unknown command: {raw}"
    end
end

let srv = http_listen("127.0.0.1:8090")
cli_info("Discord bot local bridge listening on :8090")`,
    classicCode: `fn handle_command(raw: text) -> text
    let cmd = lower(trim(raw))
    if cmd == "!ping"
        return $"Pong! Alive on {os_name()}/{os_arch()}"
    otherwise if cmd == "!roll"
        return $"Rolled a {rand_int(1, 20)} (d20)"
    otherwise
        return $"Unknown command: {raw}"
    end
end

let srv = http_listen("127.0.0.1:8090")
cli_info("Discord bot local bridge listening on :8090")`,
  },
  errors: {
    label: "Strict Diagnostics",
    file: "docs/ERROR_CODES.md",
    notes: [
      "Strict typing catches silent `text + number` coercion bugs before code ever runs (`E021`).",
      "Beginner-friendly `E001` hints automatically guide users from `!`, `&&`, `||`, `;` to `not`, `and`, `or`.",
      "Runtime errors carry formatted `STACK TRACE:` frames and clean `try ... catch err ... end` recovery.",
    ],
    englishCode: `try
    let input_age = "not-a-number"
    let parsed = number(input_age)
    print($"Next year: {parsed + 1}")
catch err
    cli_warn($"Handled safely: {err}")
end`,
    classicCode: `try
    let input_age = "not-a-number"
    let parsed = number(input_age)
    say($"Next year: {parsed + 1}")
catch err
    cli_warn($"Handled safely: {err}")
end`,
  },
};

export const PLAYGROUND_PRESETS = [
  {
    id: "english_tour",
    label: "English Tour",
    code: `# Try editing this DOG program and clicking Run Code!
define greet(name: text, title: text = "Friend") returns text
    return $"Hello, {title} {name}!"
end

var points = 0
let badge = "Starter"
print(greet("Alex"))

if badge is "Starter" then
    increase points by 10
elif badge is "Pro" then
    increase points by 25
else
    set points to 5
end

unless points is less than 10 then
    print($"Nice! You have {points} points (rank: {badge}).")
end

repeat 2 times
    print("Keep learning DOG!")
end

for step from 1 to 3
    print($"Countdown step {step}")
end

var retries = 0
until retries is 2
    increase retries by 1
end

print($"Done after {retries} retries.")`,
  },
  {
    id: "functions_lists",
    label: "Functions & Collections",
    code: `define score_summary(items: list<number>) returns text
    let total = list_sum(items)
    let avg = list_avg(items)
    return $"Items: {len(items)} | Total: {total} | Avg: {avg}"
end

let results = [88, 94, 79, 97]
print(score_summary(results))

for each s in results
    if s is at least 90 then
        cli_success($"Honors score: {s}")
    else
        cli_info($"Standard score: {s}")
    end
end`,
  },
  {
    id: "error_catch",
    label: "Try / Catch Recovery",
    code: `try
    let raw = "forty-two"
    let parsed = number(raw)
    print($"Parsed value: {parsed}")
catch err
    cli_warn($"Caught error cleanly: {err}")
end

cli_progress("Verification", 3, 3)`,
  },
  {
    id: "strict_type_err",
    label: "Strict Type Error (E021)",
    code: `# DOG catches silent text + number coercion at check time.
# Fix it by changing "Age: " + age to $"Age: {age}"!
let age = 21
print("Age: " + age)`,
  },
];

export const INSTALL_METHODS = [
  {
    id: "termux_apt",
    label: "Android Termux (APT)",
    subtitle: "Prebuilt .deb packages for aarch64, arm (ARMv7), and x86_64 via GitHub Pages APT repository.",
    commands: `curl -sSL https://raw.githubusercontent.com/kaydankotze17-lang/Dog/main/packaging/termux/setup-apt.sh | bash
dog learn
dog run examples/english_basics.dg`,
    altLabel: "Or add the repository manually:",
    altCommands: `echo "deb [trusted=yes] https://kaydankotze17-lang.github.io/Dog termux main" > $PREFIX/etc/apt/sources.list.d/dog.list
pkg update && pkg install -y dog`,
  },
  {
    id: "termux_source",
    label: "Termux from Source",
    subtitle: "Zero external crate dependencies—compiles cleanly with stock Cargo & Rust >= 1.70 on Android Termux.",
    commands: `pkg update && pkg install -y rust git
git clone https://github.com/kaydankotze17-lang/Dog.git
cd Dog
cargo test
cargo install --path .`,
    altLabel: "Start interactive lessons & mobile editor immediately:",
    altCommands: `dog learn english --run
dog studio .`,
  },
  {
    id: "desktop",
    label: "Linux & macOS",
    subtitle: "Build release binary directly with Cargo for x86_64 or Apple Silicon hosts.",
    commands: `git clone https://github.com/kaydankotze17-lang/Dog.git
cd Dog
cargo build --release
cargo install --path .
dog --version`,
    altLabel: "Compile standalone bundles or native executables:",
    altCommands: `dog build examples/web_server.dg --out dist/server.dgb
dog build examples/hello.dg --native`,
  },
];

export const CLI_COMMANDS = [
  { cmd: "dog learn [topic] [--run]", desc: "7 interactive beginner lessons with live execution (`basics`, `english`, `input`, `functions`, `data`, `errors`, `web`)." },
  { cmd: "dog web <build|css|serve>", desc: "Compile `.dg` browser apps + `.dgss` DOG CSS stylesheets into standalone HTML/CSS/JS websites." },
  { cmd: "dog run <file.dg | file.dgb>", desc: "Run scripts or SHA-256 verified bytecode bundles with stack traces." },
  { cmd: "dog studio [dir]", desc: "Split-pane terminal IDE with live IntelliSense, auto soft word-wrap (<72 cols), and Explorer." },
  { cmd: "dog edit [file.dg]", desc: "256-color mobile-friendly editor with contextual suggestions, snippets, and formatting." },
  { cmd: "dog debug <file.dg>", desc: "Interactive step, breakpoint, and variable-inspector debugger (`n`, `c`, `b <line>`, `p`)." },
  { cmd: "dog watch <file.dg>", desc: "Monitor entry script and transitive `use` dependencies and auto-rerun on changes." },
  { cmd: "dog server [dir] [port]", desc: "Multi-threaded static HTTP file server for local and LAN hosting." },
];

export const WEB_STUDIO_PRESETS = [
  {
    id: "interactive_counter",
    label: "Interactive Site & Notes",
    title: "DOG Interactive Site",
    code: `# Client-side interactive website written in DOG!
use "std.web"

var clicks = 0
var accent_on = false

define render_ui() returns void
    let nav = web_nav("My DOG Site", {"Counter": "#counter", "Notes": "#notes"})
    let hero = web_card("Interactive Counter", $"Button clicks: <strong id=\\"count-val\\" data-testid=\\"count\\">{clicks}</strong><div style=\\"margin-top:12px;display:flex;gap:10px;flex-wrap:wrap;\\">{web_button("inc-btn", "+1 Click")}{web_button("mode-btn", "Toggle Theme")}</div>")
    let form = web_card("Live Note Box", $"<div style=\\"display:grid;gap:10px;\\">{web_input("note-box", "Type a banner message...", "")}{web_button("save-btn", "Update Banner")}</div>")
    let status = html_el("p", {"id": "status-bar", "class": "dg-banner"}, "Click a button or type a note to test DOG DOM events.")
    dom_mount("#app", html_el("main", {"id": "shell", "class": "dg-shell"}, $"{nav}{hero}{form}{status}"))
    dom_on("#inc-btn", "click", "on_increment")
    dom_on("#mode-btn", "click", "on_toggle_mode")
    dom_on("#save-btn", "click", "on_save_note")
end

define on_increment() returns void
    increase clicks by 1
    dom_set_text("#count-val", text(clicks))
    dom_set_text("#status-bar", $"DOG click handler fired! Clicks = {clicks}")
    print($"Clicks updated to {clicks}")
end

define on_toggle_mode() returns void
    set accent_on to dom_toggle_class("#shell", "is-warm")
    if accent_on then
        dom_set_text("#status-bar", "Warm accent canvas enabled via dom_toggle_class().")
    else
        dom_set_text("#status-bar", "Clean paper canvas restored.")
    end
end

define on_save_note() returns void
    let note = trim(dom_get_value("#note-box"))
    if note is "" then
        dom_set_text("#status-bar", "Enter text in the input field first.")
    else
        dom_set_text("#status-bar", $"Updated from input: {note}")
        web_storage_set("dog_demo_note", note)
        dom_set_value("#note-box", "")
    end
end

render_ui()`,
    css: `# DOG Style Sheet (.dgss) compiled directly to browser CSS
vars
    bg: #F7F6F2
    surface: #FFFFFF
    ink: #181A1F
    accent: #C85A11
    accent-hover: #A6480B
    border: #E2DFD7
    radius: 10px
end

rule *
    box-sizing: border-box
end

rule body
    margin: 0
    bg: $bg
    ink: $ink
    font: system-ui, -apple-system, sans-serif
    leading: 1.6
end

rule .dg-shell
    max-w: 680px
    mx: auto
    pad: 20px
    flex: col
    gap: 14px
    radius: $radius
    ease: background 180ms ease
end

rule .dg-shell.is-warm
    bg: #FDF2EA
    border: 1px solid $accent
end

rule .dg-nav
    display: flex
    justify: space-between
    align: center
    pad: 12px 16px
    bg: $surface
    border: 1px solid $border
    radius: $radius
end

rule .dg-nav-links
    display: flex
    gap: 14px
end

rule .dg-nav a
    ink: $ink
    text-decoration: none
    weight: 600
end

rule .dg-card
    bg: $surface
    border: 1px solid $border
    radius: $radius
    pad: 16px
    shadow: sm
end

rule .dg-card h3
    margin: 0 0 8px
    size: 1.05rem
end

rule .dg-btn
    bg: $accent
    ink: #FFFFFF
    border: none
    radius: 6px
    py: 8px
    px: 14px
    weight: 600
    cursor: pointer

    on hover
        bg: $accent-hover
    end
end

rule .dg-input
    w: 100%
    py: 8px
    px: 12px
    radius: 6px
    border: 1px solid $border
    bg: $surface
    ink: $ink
end

rule .dg-banner
    pad: 10px 14px
    bg: $surface
    border: 1px solid $border
    radius: 6px
    weight: 500
    margin: 0
end

on mobile
    rule .dg-shell
        pad: 12px
    end
end`,
  },
  {
    id: "landing_showcase",
    label: "Studio Landing Page",
    title: "Built with DOG Web",
    code: `use "std.web"

var selected = "Starter"

define choose_starter() returns void
    set selected to "Starter"
    dom_set_text("#plan-out", "Selected plan: Starter ($0/mo local static host)")
end

define choose_pro() returns void
    set selected to "Studio Pro"
    dom_set_text("#plan-out", "Selected plan: Studio Pro (Full HTTP + WebSocket API)")
end

let cards = web_grid(2, [
    web_card("Starter", $"Pure static HTML/CSS/JS bundle compiled by DOG.<div style=\\"margin-top:12px\\">{web_button("pick-a", "Pick Starter")}</div>"),
    web_card("Studio Pro", $"Live HTTP routes, WebSockets & KV database built in.<div style=\\"margin-top:12px\\">{web_button("pick-b", "Pick Studio Pro")}</div>")
])
let banner = html_el("p", {"id": "plan-out", "class": "dg-banner"}, $"Active choice: {selected}")
dom_mount("#app", html_el("main", {"class": "dg-shell"}, $"{cards}{banner}"))
dom_on("#pick-a", "click", "choose_starter")
dom_on("#pick-b", "click", "choose_pro")`,
    css: `vars
    bg: #16181D
    surface: #1E2128
    ink: #ECEFF4
    accent: #C85A11
    border: #2B2F3A
end

rule *
    box-sizing: border-box
end

rule body
    margin: 0
    bg: $bg
    ink: $ink
    font: system-ui, sans-serif
end

rule .dg-shell
    max-w: 720px
    mx: auto
    pad: 22px
    flex: col
    gap: 16px
end

rule .dg-grid
    gap: 16px
end

rule .dg-card
    bg: $surface
    border: 1px solid $border
    radius: 10px
    pad: 18px
end

rule .dg-btn
    bg: $accent
    ink: #FFFFFF
    border: none
    radius: 6px
    py: 8px
    px: 14px
    weight: 600
    cursor: pointer
end

rule .dg-banner
    bg: $surface
    border: 1px solid $border
    radius: 8px
    pad: 12px 16px
    margin: 0
end

on mobile
    rule .dg-grid
        cols: 1
    end
end`,
  },
];
