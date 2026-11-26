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
      { name: "http_req / http_res / openapi_spec", sig: "... -> map | text", desc: "Typed HTTP builders, route groups, rate limiting, and OpenAPI JSON generation." },
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
      { name: "map / filter / reduce / query_where", sig: "(list<T>, ...) -> list | T", desc: "Functional transforms and map-list equality filtering." },
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
      { name: "json_validate(raw, required_keys)", sig: "(text, list<text>) -> bool", desc: "Validate JSON object payloads against required schema keys." },
      { name: "json_stringify(val) / json_pretty(val)", sig: "any -> text", desc: "Serialize DOG maps, lists, or structs to JSON." },
      { name: "json_write(path, val)", sig: "(text, any) -> void", desc: "Pretty-write DOG data structures directly to disk." },
      { name: "toml_parse(raw)", sig: "text -> map<text, text>", desc: "Parse flattened TOML sections and key/value pairs." },
      { name: "csv_parse(raw) / csv_stringify(rows)", sig: "text <-> list<list<text>>", desc: "RFC 4180 CSV parser and serializer." },
      { name: "kv_set(db, k, v) / kv_get(db, k) / kv_keys(db)", sig: "(text, ...) -> ...", desc: "Persistent local JSON-backed key-value storage." },
      { name: "db_open / db_exec / db_query / db_begin / db_rollback", sig: "(db, sql, params?) -> list | number", desc: "Embedded SQLite engine with prepared `?` params, transactions, and `query_builder`." },
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
      { name: "hmac_sha256_verify(key, msg, sig) / secure_eq(a, b)", sig: "(text, ...) -> bool", desc: "Constant-time HMAC and secret comparison." },
      { name: "pbkdf2_sha256(pass, salt, rounds)", sig: "(text, text, number) -> text", desc: "Iterative PBKDF2-HMAC-SHA256 key derivation." },
      { name: "base64_encode(s) / base64_decode(s)", sig: "text -> text", desc: "Standard Base64 and Base64URL encoding/decoding." },
      { name: "hex_encode(s) / hex_decode(h)", sig: "text -> text", desc: "Lowercase hex encoding and validated decoding." },
      { name: "rand_bytes_hex(count)", sig: "number -> text", desc: "Generate cryptographically random hex bytes." },
      { name: "password_hash / password_verify", sig: "(text, text?) -> text | bool", desc: "Salted PBKDF2-HMAC-SHA256 password hashing and constant-time verification." },
      { name: "encrypt_aes256 / decrypt_aes256", sig: "(text, text) -> text", desc: "HMAC-authenticated AES-256 symmetric encryption and decryption." },
      { name: "ed25519_keypair / ed25519_sign / ed25519_verify", sig: "... -> map | text | bool", desc: "RFC 8032 Ed25519 digital keypairs, signatures, and verification." },
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
      { name: "web_form / web_field / web_alert / web_badge", sig: "... -> text", desc: "Accessible form inputs, labels, ARIA alerts, and tone badges." },
      { name: "web_validate_required / web_validate_email / dom_focus", sig: "text -> bool | void", desc: "Client form validation with aria-invalid + focus management." },
      { name: "web_storage_set(k, v) / web_storage_get(k)", sig: "(text, ...) -> void | text", desc: "Read and persist browser `localStorage` state." },
      { name: "web_component / web_state_set / dom_delegate", sig: "... -> void | text", desc: "Reusable components, reactive state watchers, event delegation, tabs, and modals." },
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
    simpleCode: `# Ultra-short Simple Style (zero type noise, same strict safety)
do greet(name, title = "Friend") = $"Hello, {title} {name}!"

make points = 0
keep badge = "Starter"
show greet("Alex")

when badge is "Starter"
    add 10 to points
or when badge is "Pro"
    add 25 to points
else
    points = 5
end

when points >= 10
    show $"Nice! {points} pts (rank: {badge})"
end

each step in 1..3
    show $"Countdown step {step}"
end`,
  },
  web_css: {
    label: "Website & CSS",
    file: "examples/web_css_site.dg",
    notes: [
      "Define theme tokens and scoped styles inline with `style theme ... end` or `style \".card\" ... end`—no raw CSS files required.",
      "Attach nested interactive states (`on hover ... end`) and responsive breakpoints (`on mobile ... end`) directly inside DOG.",
      "Mount XSS-escaped UI components with `dom_mount()` and bind DOG functions to DOM clicks with `dom_on()`.",
    ],
    englishCode: `use "std.web"

style theme
    bg: "#F7F6F2"
    surface: "#FFFFFF"
    accent: "#C85A11"
end

style ".dg-card"
    bg: "$surface"
    pad: "20px"
    radius: "10px"
    shadow: "sm"
    on hover
        shadow: "md"
    end
    on mobile
        pad: "14px"
    end
end

var clicks = 0
define greet_click() returns void
    increase clicks by 1
    dom_set_text("#status", $"Clicked {clicks} time(s) in pure DOG!")
end

let card = web_card("Hero", $"Built natively in DOG! {web_button("hi-btn", "Say Hi")}")
dom_mount("#app", card)
dom_on("#hi-btn", "click", "greet_click")`,
    classicCode: `use "std.web"

style ":root"
    bg: "#F7F6F2"
    surface: "#FFFFFF"
    accent: "#C85A11"
end

style ".dg-card"
    bg: "$surface"
    pad: "20px"
    radius: "10px"
    shadow: "sm"
    hover
        shadow: "md"
    end
    mobile
        pad: "14px"
    end
end

var clicks = 0
fn greet_click() -> void
    clicks += 1
    dom_set_text("#status", $"Clicked {clicks} time(s) in pure DOG!")
end

let card = web_card("Hero", $"Built natively in DOG! {web_button("hi-btn", "Say Hi")}")
dom_mount("#app", card)
dom_on("#hi-btn", "click", "greet_click")`,
    simpleCode: `use "std.web"

style theme
    bg: "#F7F6F2"
    surface: "#FFFFFF"
    accent: "#C85A11"
end

style ".dg-card"
    bg: "$surface"
    pad: "20px"
    radius: "10px"
    shadow: "sm"
    on hover
        shadow: "md"
    end
end

make clicks = 0
do greet_click()
    bump clicks
    dom_set_text("#status", $"Clicked {clicks} time(s) in Simple Style!")
end

keep card = web_card("Hero", $"Built in Simple DOG! {web_button("hi-btn", "Say Hi")}")
dom_mount("#app", card)
dom_on("#hi-btn", "click", "greet_click")`,
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
    simpleCode: `keep srv = http_listen("127.0.0.1:8080")
cli_info($"Serving on http://127.0.0.1:{http_port(srv)}")

loop
    keep req = http_accept(srv)
    keep path = map_opt(req, "path") ?? "/"

    when path == "/"
        http_reply(req, 200, "text/html", "<h1>DOG Simple Server</h1>")
    or when path == "/api/status"
        http_reply(req, 200, "application/json", json_stringify({"ok": "true", "os": os_name()}))
    else
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
    simpleCode: `do handle_command(raw)
    keep cmd = raw |> trim |> lower
    when cmd == "!ping"
        give $"Pong! Alive on {os_name()}/{os_arch()}"
    or when cmd == "!roll"
        give $"Rolled a {rand_int(1, 20)} (d20)"
    else
        give $"Unknown command: {raw}"
    end
end

keep srv = http_listen("127.0.0.1:8090")
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
    simpleCode: `try
    keep input_age = "not-a-number"
    keep parsed = number(input_age)
    show $"Next year: {parsed + 1}"
oops err
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
    id: "simple_tour",
    label: "Simple Style Tour (Ultra-Short)",
    code: `# Simple Style: zero type boilerplate, shortest keywords, 100% mixable!
do greet(name, title = "Friend") = $"Hello, {title} {name}!"

do double(n) = n * 2

make score = 10
bump score by 15
keep msg = greet("Maya") |> trim |> upper
show msg

when score >= 25
    show $"Level Up! score={score}"
else
    show "Keep going"
end

make sum = 0
each i in 1..5
    when i == 2
        skip
    end
    add i to sum
end
show $"Range sum (skip 2) = {sum}"

keep doubled = [1, 2, 3, 4] |> list_chunk(2)
keep cfg = {"mode": "simple"}
keep port = map_opt(cfg, "port") ?? "8080"
show $"Chunk0={join(doubled[0], ",")} | Port={port} | 7*2={double(7)}"

try
    keep _n = number("bad")
oops err
    show $"Recovered safely via oops: {err}"
end`,
  },
  {
    id: "interop_all_styles",
    label: "All 3 Styles Together (Interop)",
    code: `# Mix English, Classic, and Simple Style freely in the same file, function, and block!
define calc_eng(base: number, bonus: number = 5) returns number
    make total = base
    increase total by bonus
    give total
end

fn double_classic(n: number) -> number
    return n * 2
end

do add_ten(x) = x + 10

var score = calc_eng(10)
keep bonus = double_classic(4)
add bonus to score

# Single conditional mixing if / or when / elif / otherwise
if score >= 30 then
    show $"Elite tier score={score}"
or when score >= 20
    say $"Pro tier score={score}"
elif score >= 10
    print("Starter tier")
else
    print("Keep going")
end

# Mixed loop styles sharing the same mutable binding
for step in 1..2
    bump score
end
each extra from 1 to 2
    score += extra
end

keep piped = score |> add_ten |> do(v) => v * 2
check piped == 76
print($"Interop OK! score={score}, piped={piped}")`,
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
  {
    id: "v1_showcase",
    label: "v1.0 Showcase (Crypto, Data, Routes & Tests)",
    code: `# Full v1.0.0 capabilities in one self-contained script!
type UserId = text

let ids = list_unique(["u-01", "u-02", "u-01"])
let sig = hmac_sha256("dog-key", "deploy:v1.0.0")
let verified = hmac_sha256_verify("dog-key", "deploy:v1.0.0", sig)
assert_eq(verified, true)

let payload = json_stringify({"service": "dog-api", "version": "1.0.0"})
assert_eq(json_validate(payload, ["service", "version"]), true)

let route = http_route_match("/users/:id", "/users/u-01")
let target = path_normalize("src/../src/main.dg")

print($"IDs={len(ids)} | HMAC ok={verified} | Route user={map_get(route, "id")}")
print($"Normalized path: {target}")
snapshot_check("showcase-ok", $"sha256={sha256("dog")}")`,
  },
  {
    id: "type_aliases_functional",
    label: "Type Aliases, Destructuring & Functional",
    code: `# Type aliases, destructuring, inline conditionals, and W001 lint warnings
type Score = number

define double_num(n: number) returns number
    return n * 2
end

define is_high(n: number) returns bool
    return n >= 80
end

let [top, runner_up] = [95, 82, 64]
let raw = [40, 85, 92]
let passing = filter(raw, "is_high")
let doubled = map(passing, "double_num")
let tier = if top >= 90 then "Honors" else "Pass"

print($"Top={top}, Second={runner_up}, Tier={tier}")
print($"Doubled passing scores: {join(doubled, ", ")}")`,
  },
  {
    id: "structs_enums_match",
    label: "Structs, Enums, Match & Lambdas",
    code: `# Structs, algebraic enums, match guards, slices, and arrow lambdas
struct Endpoint
    name: text
    port: number
end

enum Status
    Ready(number)
    Failed(text)
    Idle
end

var svc = Endpoint { name: "dog-api", port: 8080 }
set svc.port to 8443

fn format_status(s: Status) -> text
    match s
        case Status.Ready(code) if code >= 200
            return $"OK ({code})"
        case Status.Failed(msg)
            return $"ERR: {msg}"
        else
            return "IDLE"
    end
end

fn make_multiplier(factor: number) -> fn
    return fn(n) => n * factor
end

let nums = [10, 20, 30, 40]
let sliced = nums[1..3]
let scaled = map(sliced, make_multiplier(3))
print($"Service={svc.name}:{svc.port} | {format_status(Status.Ready(204))}")
print($"Sliced={join(sliced, ",")} | Scaled={join(scaled, ",")}")`,
  },
  {
    id: "db_crypto_tasks",
    label: "SQLite, Ed25519, AES-256 & Tasks",
    code: `# Relational SQLite, authenticated encryption, Ed25519, and concurrent tasks
let db = db_open(":memory:")
db_exec(db, "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT, qty INTEGER)")
db_exec(db, "INSERT INTO items (label, qty) VALUES (?, ?)", ["bolts", 12])
db_begin(db)
db_exec(db, "INSERT INTO items (label, qty) VALUES (?, ?)", ["temp-item", 99])
db_rollback(db)

let rows = db_query(db, "SELECT label, qty FROM items WHERE qty > ?", [5])
let cipher = encrypt_aes256("dog-secret", "token=ok")
let plain = decrypt_aes256("dog-secret", cipher)
let kp = ed25519_keypair("demo")
let sig = ed25519_sign(kp["private_key"], plain)
let sig_ok = ed25519_verify(kp["public_key"], plain, sig)

let tasks = [spawn_task(fn(x) => x * 10, 4), spawn_task(fn(x) => x * 10, 7)]
let done = await_all(tasks)
print($"Rows={len(rows)} | AES={plain} | Ed25519={sig_ok} | Tasks={join(done, ",")}")`,
  },
];

export const INSTALL_METHODS = [
  {
    id: "termux_apt",
    label: "Android Termux (Bulletproof Updater)",
    subtitle: "Zero-error Termux install & upgrade: purges stale DOG APT caches, rejects HTML 404s, verifies .deb/SHA-256, backs up & rolls back cleanly.",
    commands: `curl -fsSL https://kaydankotze17-lang.github.io/Dog/update.sh | bash
dog --version
dog learn`,
    altLabel: "Check versions, force reinstall, or upgrade directly from CLI:",
    altCommands: `dog update
dog update --check
dog update --force`,
  },
  {
    id: "termux_source",
    label: "Termux from Source",
    subtitle: "Zero external crate dependencies—compiles cleanly with stock Cargo & Rust >= 1.70 on Android Termux.",
    commands: `pkg update && pkg install -y rust git
git clone https://github.com/kaydankotze17-lang/Dog.git
cd Dog
cargo test
cargo install --path . --force`,
    altLabel: "Pull newest main commits and rebuild anytime:",
    altCommands: `dog update --from-source
dog studio .`,
  },
  {
    id: "desktop",
    label: "Linux & macOS",
    subtitle: "Build release binary directly with Cargo for x86_64 or Apple Silicon hosts.",
    commands: `git clone https://github.com/kaydankotze17-lang/Dog.git
cd Dog
cargo build --release
cargo install --path . --force
dog --version`,
    altLabel: "Compile standalone websites, bundles, or update automatically:",
    altCommands: `dog web build examples/web_site.dg --out dist_web
dog build examples/hello.dg --native
dog update`,
  },
];

export const CLI_COMMANDS = [
  { cmd: "dog update [--check|--force|--from-source]", desc: "Compare installed CLI vs GitHub main & Termux APT, purge stale caches, verify .deb/SHA-256, and upgrade with atomic rollback." },
  { cmd: "dog learn [topic] [--run]", desc: "7 interactive beginner lessons with live execution (`basics`, `english`, `input`, `functions`, `data`, `errors`, `web`)." },
  { cmd: "dog web <build|css|serve>", desc: "Compile `.dg` browser apps + inline `style` / `.dgss` DOG CSS stylesheets into standalone HTML/CSS/JS websites." },
  { cmd: "dog run <file.dg | file.dgb>", desc: "Run scripts or SHA-256 verified bytecode bundles with stack traces." },
  { cmd: "dog studio [dir]", desc: "Split-pane terminal IDE with live IntelliSense, auto soft word-wrap (<72 cols), and Explorer." },
  { cmd: "dog edit [file.dg]", desc: "256-color mobile-friendly editor with contextual suggestions, snippets, and formatting." },
  { cmd: "dog debug <file.dg>", desc: "Interactive step, breakpoint, and variable-inspector debugger (`n`, `c`, `b <line>`, `p`)." },
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
  {
    id: "inline_style_site",
    label: "Single-File Inline CSS (.dg only)",
    title: "Inline DOG Style Site",
    code: `# Zero separate .dgss needed — write style "..." directly inside .dg!
use "std.web"

style theme
    bg: "#F7F6F2"
    surface: "#FFFFFF"
    ink: "#181A1F"
    accent: "#C85A11"
end

style "body"
    m: "0"
    bg: "$bg"
    ink: "$ink"
    font: "system-ui, sans-serif"
end

style ".dg-shell"
    max-w: "640px"
    mx: "auto"
    pad: "20px"
    flex: "col"
    gap: "14px"
end

style ".dg-card"
    bg: "$surface"
    border: "1px solid #E2DFD7"
    radius: "10px"
    pad: "18px"
    shadow: "sm"
    on hover
        shadow: "md"
    end
end

style ".dg-btn"
    bg: "$accent"
    ink: "#FFFFFF"
    border: "none"
    radius: "6px"
    py: "9px"
    px: "14px"
    weight: "600"
    cursor: "pointer"
end

var likes = 0

define add_like() returns void
    increase likes by 1
    dom_set_text("#like-count", text(likes))
    print($"Likes = {likes}")
end

let card = web_card("Styled Inline in DOG", $"Votes: <strong id=\\"like-count\\">{likes}</strong><div style=\\"margin-top:12px\\">{web_button("like-btn", "Upvote")}</div>")
dom_mount("#app", html_el("main", {"class": "dg-shell"}, card))
dom_on("#like-btn", "click", "add_like")`,
    css: "",
  },
  {
    id: "accessible_form",
    label: "Accessible Form, Validation & Autoprefix",
    title: "DOG Accessible Form",
    code: `# Accessible UI builders + ARIA validation + submit preventDefault
use "std.web"

define render_form() returns void
    let badge = web_badge("v1.0 Ready", "success")
    let fields = $"{web_field("contact-name", "Your Name", "Ada Lovelace", "text")}{web_field("contact-email", "Email Address", "ada@example.org", "email")}"
    let form = web_form("signup-form", fields, "Create Account")
    let card = web_card("Sign Up", $"<div style=\\"margin-bottom:10px;\\">{badge}</div>{form}")
    let notice = html_el("div", {"id": "form-notice"}, web_alert("Ready", "Fill both fields and submit.", "info"))
    dom_mount("#app", html_el("main", {"class": "dg-shell"}, $"{card}{notice}"))
    dom_on("#signup-form", "submit", "on_submit_form")
end

define on_submit_form() returns void
    let ok_name = web_validate_required("#contact-name")
    let ok_mail = web_validate_email("#contact-email")
    if not ok_name then
        dom_set_html("#form-notice", web_alert("Missing Name", "Please enter your name.", "error"))
        dom_focus("#contact-name")
        return
    end
    if not ok_mail then
        dom_set_html("#form-notice", web_alert("Invalid Email", "Enter a valid email address.", "error"))
        dom_focus("#contact-email")
        return
    end
    let who = trim(dom_get_value("#contact-name"))
    dom_set_html("#form-notice", web_alert("Saved", $"Welcome aboard, {who}!", "success"))
    print($"Account created for {who}")
end

render_form()`,
    css: `vars
    bg: #F7F6F2
    surface: #FFFFFF
    ink: #181A1F
    accent: #C85A11
    border: #E2DFD7
end

rule body
    margin: 0
    bg: $bg
    ink: $ink
    font: system-ui, sans-serif
end

rule .dg-shell
    max-w: 560px
    mx: auto
    pad: 20px
    flex: col
    gap: 14px
end

rule .dg-card
    bg: $surface
    border: 1px solid $border
    radius: 10px
    pad: 18px
    backdrop-filter: blur(6px)
end

rule .dg-form
    flex: col
    gap: 12px
end

rule .dg-field
    flex: col
    gap: 6px
end

rule .dg-label
    weight: 600
    size: 0.88rem
end

rule .dg-input
    w: 100%
    py: 8px
    px: 12px
    radius: 6px
    border: 1px solid $border
end

rule .dg-btn
    bg: $accent
    ink: #FFFFFF
    border: none
    radius: 6px
    py: 9px
    px: 14px
    weight: 600
    cursor: pointer
    user-select: none
end

rule .dg-badge
    display: inline-block
    px: 8px
    py: 2px
    radius: 4px
    size: 0.78rem
    bg: #EAF7EE
    ink: #166534
end

rule .dg-alert
    pad: 10px 14px
    radius: 6px
    border: 1px solid $border
    bg: $surface
end`,
  },
  {
    id: "reactive_components_treeshake",
    label: "Reactive Components & CSS Tree-Shake",
    title: "DOG Reactive Components",
    code: `# Reusable web_component, reactive state watchers, dom_delegate & extend inheritance
use "std.web"

define render_summary(props: map) returns text
    let label = map_get(props, "label")
    let val = map_get(props, "value")
    return web_card("Active Module", $"<strong>{label}</strong><p style=\\"margin:8px 0 0\\">{val}</p>")
end

define on_tab_selected(tab_key: text) returns void
    web_state_set("active_tab", tab_key)
    dom_set_text("#tab-status", $"Delegated tab switch -> {tab_key}")
end

define render_app() returns void
    web_component("summary-box", "render_summary")
    let tabs = web_tabs("main-tabs", {"overview": "Overview", "crypto": "Crypto", "db": "SQLite"}, "overview")
    let box = html_el("div", {"id": "panel-host"}, web_render_component("summary-box", {"label": "overview", "value": "Reactive state & extend: .dg-card active."}))
    let status = html_el("p", {"id": "tab-status", "class": "dg-card dg-card-accent"}, "Click any tab above to trigger dom_delegate + web_state_set.")
    dom_mount("#app", html_el("main", {"class": "dg-shell"}, $"{tabs}{box}{status}"))
    dom_delegate("#main-tabs", ".dg-tab-btn", "click", "on_tab_selected")
end

web_on_state("active_tab", "on_state_log")

define on_state_log(next_val: text) returns void
    dom_set_html("#panel-host", web_render_component("summary-box", {"label": next_val, "value": $"State updated cleanly to '{next_val}'."}))
    print($"State active_tab -> {next_val}")
end

render_app()`,
    css: `vars
    bg: #F7F6F2
    surface: #FFFFFF
    ink: #181A1F
    accent: #C85A11
    border: #E2DFD7
end

rule body
    margin: 0
    bg: $bg
    ink: $ink
    font: system-ui, sans-serif
end

rule .dg-shell
    max-w: 620px
    mx: auto
    pad: 20px
    flex: col
    gap: 14px
end

rule .dg-card
    bg: $surface
    border: 1px solid $border
    radius: 10px
    pad: 16px
    shadow: sm
end

rule .dg-card-accent
    extend: .dg-card
    border: 1px solid $accent
end

rule .dg-tabs
    display: flex
    gap: 8px
end

rule .dg-tab-btn
    bg: $surface
    ink: $ink
    border: 1px solid $border
    radius: 6px
    py: 8px
    px: 14px
    weight: 600
    cursor: pointer
end

rule .unused-legacy-banner
    bg: #181A1F
    ink: #FFFFFF
    pad: 24px
end`,
  },
  {
    id: "simple_web_app",
    label: "Simple Style Web App (Table + Toast)",
    title: "Simple Style Task Board",
    code: `# Build websites in Simple Style: keep, make, do, when, show, bump
use "std.web"

make done_count = 1
keep total_count = 3

do render_board()
    keep pct = round((done_count * 100) / total_count)
    keep rows = [
        ["Simple Style Parser", "Ready"],
        ["Pipe |> & Nullish ??", "Ready"],
        ["Interactive Task", if done_count >= 2 then "Done" else "Pending"]
    ]
    keep tbl = web_table(["Module", "Status"], rows)
    keep prog = web_progress("board-prog", done_count, total_count)
    keep btn = web_button("bump-btn", "Complete Task (+1)")
    keep body = $"<div style=\\"margin-bottom:12px\\">{prog}</div>{tbl}<div style=\\"margin-top:14px;display:flex;gap:10px;align-items:center;\\">{btn}<span id=\\"pct-label\\">{pct}% complete</span></div>"
    keep card = web_card("Simple Style Board", body)
    keep toast = html_el("div", {"id": "toast-host"}, "")
    dom_mount("#app", html_el("main", {"class": "dg-shell"}, $"{card}{toast}"))
    dom_on("#bump-btn", "click", "on_complete_task")
end

do on_complete_task()
    when done_count < total_count
        bump done_count
    else
        done_count = 1
    end
    render_board()
    dom_set_html("#toast-host", web_toast($"Updated progress: {done_count}/{total_count}", "success"))
    show $"Task count -> {done_count}/{total_count}"
end

render_board()`,
    css: `vars
    bg: #F7F6F2
    surface: #FFFFFF
    ink: #181A1F
    accent: #C85A11
    border: #E2DFD7
end

rule body
    margin: 0
    bg: $bg
    ink: $ink
    font: system-ui, sans-serif
end

rule .dg-shell
    max-w: 660px
    mx: auto
    pad: 20px
    flex: col
    gap: 14px
end

rule .dg-card
    bg: $surface
    border: 1px solid $border
    radius: 10px
    pad: 18px
    shadow: sm
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
end`,
  },
];
