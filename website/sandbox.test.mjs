import test from "node:test";
import assert from "node:assert/strict";
import { runDogSandbox } from "./sandbox.js";

test("executes English-style functions, defaults, loops, and conditionals", () => {
  const code = `
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
    print($"Points = {points}")
end

repeat 2 times
    increase points by 1
end

for step from 1 to 3
    increase points by step
end

var retries = 0
until retries is 2
    increase retries by 1
end

print($"Final points: {points}, retries: {retries}")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, [
    "Hello, Friend Alex!",
    "Points = 10",
    "Final points: 18, retries: 2",
  ]);
});

test("enforces strict typing (E021) and immutability (E012)", () => {
  const typeErr = runDogSandbox('let age = 20\nprint("Age: " + age)\n');
  assert.equal(typeErr.ok, false);
  assert.equal(typeErr.errorCode, "E021");

  const mutErr = runDogSandbox("let fixed = 5\nincrease fixed by 1\n");
  assert.equal(mutErr.ok, false);
  assert.equal(mutErr.errorCode, "E012");
});

test("emits beginner English hints for !, &&, || and supports try/catch", () => {
  const bangErr = runDogSandbox("let ok = !true\n");
  assert.equal(bangErr.ok, false);
  assert.equal(bangErr.errorCode, "E001");
  assert.ok(bangErr.error.includes("not condition"));

  const caught = runDogSandbox(`
try
    let n = number("oops")
    print(n)
catch err
    print($"Recovered: {err}")
end
`);
  assert.equal(caught.ok, true, caught.error || "");
  assert.equal(caught.output.length, 1);
  assert.ok(caught.output[0].startsWith("Recovered:"));
});

test("supports SHA-256, HMAC-SHA256, constant-time secure_eq, PBKDF2, and KV/JSON validation", () => {
  const code = `
let digest = sha256("abc")
assert_eq(digest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")

let sig = hmac_sha256("key", "The quick brown fox jumps over the lazy dog")
assert_eq(hmac_sha256_verify("key", "The quick brown fox jumps over the lazy dog", sig), true)
assert_eq(secure_eq("alpha", "alpha"), true)
assert_eq(secure_eq("alpha", "beta"), false)

let derived = pbkdf2_sha256("password", "salt", 10)
assert_eq(len(derived), 64)

let payload = json_stringify({"service": "dog", "version": "1.0.0"})
assert_eq(json_validate(payload, ["service", "version"]), true)
assert_eq(json_validate(payload, ["missing_key"]), false)

kv_set("memdb", "token", "ok-123")
assert_eq(kv_has("memdb", "token"), true)
assert_eq(kv_get("memdb", "token"), "ok-123")

let route = http_route_match("/users/:id/posts/*slug", "/users/42/posts/hello/world")
print($"id={map_get(route, "id")}|slug={map_get(route, "slug")}|norm={path_normalize("a/../b/./c.dg")}")
snapshot_check("crypto-kv", "ok")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.equal(res.output[0], "id=42|slug=hello/world|norm=b/c.dg");
  const parsed = JSON.parse(res.diagnosticsJson);
  assert.equal(parsed.ok, true);
});

test("supports type aliases, destructuring, functional combinators, inline conditionals, and W001 warnings", () => {
  const code = `
type Score = number

define double_n(x: number) returns number
    return x * 2
    let unreachable = 99
end

define is_pass(x: number) returns bool
    return x >= 50
end

define sum_acc(acc: number, x: number) returns number
    return acc + x
end

let [first, second] = [90, 45, 75]
let passed = filter([90, 45, 75], "is_pass")
let doubled = map(passed, "double_n")
let total = reduce(doubled, 0, "sum_acc")
let tier = if first >= 80 then "Honors" else "Pass"

print($"first={first}, second={second}, total={total}, tier={tier}")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, ["first=90, second=45, total=330, tier=Honors"]);
  assert.ok(res.warnings.some((w) => w.code === "W001"));
  const diag = JSON.parse(res.diagnosticsJson);
  assert.ok(diag.diagnostics.some((d) => d.code === "W001" && d.severity === "warning"));
});

test("supports structs, field mutation, algebraic enums, match guards, slices, and closures", () => {
  const code = `
struct Server
    name: text
    port: number
end

enum Reply
    Ok(number)
    Err(text)
    Empty
end

var srv = Server { name: "dog-http", port: 8080 }
set srv.port to 9000

fn describe(r: Reply) -> text
    match r
        case Reply.Ok(c) if c >= 200
            return $"OK:{c}"
        case Reply.Ok(c)
            return $"LOW:{c}"
        case Reply.Err(msg)
            return $"ERR:{msg}"
        else
            return "EMPTY"
    end
end

fn make_mul(factor: number) -> fn
    return fn(x) => x * factor
end

let raw = [10, 20, 30, 40]
let sub = raw[1..3]
let mul3 = make_mul(3)
let mapped = map(sub, mul3)
let has_big = list_any(mapped, fn(x) => x > 80)
let all_pos = list_all(mapped, fn(x) => x > 0)
let found = list_find(mapped, fn(x) => x > 70)

print($"{srv.name}:{srv.port}|{describe(Reply.Ok(204))}|{describe(Reply.Err("bad"))}|{describe(Reply.Empty)}")
print($"sub={join(sub, ",")}|mapped={join(mapped, ",")}|any={has_big}|all={all_pos}|found={found}")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, [
    "dog-http:9000|OK:204|ERR:bad|EMPTY",
    "sub=20,30|mapped=60,90|any=true|all=true|found=90",
  ]);
});

test("supports NIST SHA-512, password hashing, AES-256 auth encryption, and Ed25519 signatures", () => {
  const code = `
let sha = sha512("abc")
assert_eq(sha, "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f")

let h = password_hash("hunter2", "testsalt")
assert_eq(password_verify("hunter2", h), true)
assert_eq(password_verify("wrong", h), false)

let enc = encrypt_aes256("super-secret-key", "hello dog crypto")
assert_eq(decrypt_aes256("super-secret-key", enc), "hello dog crypto")

let kp = ed25519_keypair("seed-1")
let sig = ed25519_sign(kp["private_key"], "signed message")
assert_eq(ed25519_verify(kp["public_key"], "signed message", sig), true)
assert_eq(ed25519_verify(kp["public_key"], "tampered message", sig), false)
print("crypto-all-ok")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, ["crypto-all-ok"]);

  const badDecrypt = runDogSandbox(`
let enc = encrypt_aes256("key-1", "secret")
let out = decrypt_aes256("key-2", enc)
print(out)
`);
  assert.equal(badDecrypt.ok, false);
  assert.equal(badDecrypt.errorCode, "E030");
});

test("supports SQLite engine, prepared params, transactions, concurrency primitives, and W003/folding diagnostics", () => {
  const code = `
let unused_var = 42
let _silenced = 100
let folded = 6 * 7

let db = db_open(":memory:")
db_exec(db, "CREATE TABLE tasks (id INTEGER PRIMARY KEY, title TEXT, pts INTEGER)")
db_exec(db, "INSERT INTO tasks (title, pts) VALUES (?, ?)", ["ship-cheatsheet", 10])
db_begin(db)
db_exec(db, "INSERT INTO tasks (title, pts) VALUES (?, ?)", ["rollback-me", 99])
db_rollback(db)
db_begin(db)
db_insert(db, "tasks", {"title": "commit-me", "pts": 25})
db_commit(db)

let rows = db_query(db, "SELECT title, pts FROM tasks WHERE pts >= ? ORDER BY pts DESC", [10])
let qb = query_builder(db, "tasks", {"select": ["title"], "where": {"title": "commit-me"}})

let t1 = spawn_task(fn(n) => n + 5, 10)
let t2 = spawn_task(fn(n) => n * 2, 10)
let results = await_all([t1, t2])
let ch = channel_create()
channel_send(ch, "ping")
let atom = atomic_create(1)
let next_atom = atomic_add(atom, 4)

print($"rows={len(rows)}|qb={len(qb)}|tasks={join(results, ",")}|ch={channel_recv(ch)}|atom={next_atom}|folded={folded}")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, ["rows=2|qb=1|tasks=15,20|ch=ping|atom=5|folded=42"]);
  assert.ok(res.warnings.some((w) => w.code === "W003" && /unused_var/.test(w.title)));
  assert.ok(!res.warnings.some((w) => w.code === "W003" && /_silenced/.test(w.title)));
  assert.ok(res.optimizations.constantFolded >= 1);
});

test("supports Simple Style syntax without breaking English or Classic dialects", () => {
  const code = `
do add_bonus(n, bonus = 5) = n + bonus

do grade(score)
    when score >= 90
        give "A"
    or when score >= 75
        give "B"
    else
        give "C"
    end
end

keep [head, second] = [10, 20, 30]
make total = 0
add 5 to total
bump total by 5
take 2 from total
check total == 8

make count = 0
loop 3
    bump count
end

make rsum = 0
each i in 1..5
    when i == 2
        skip
    end
    when i == 4
        stop
    end
    add i to rsum
end

try
    keep _bad = number("abc")
oops err
    show $"head={head}|second={second}|bonus={add_bonus(10)}|grade={grade(80)}|total={total}|cnt={count}|rsum={rsum}|caught={contains(err, "Cannot") or len(err) > 0}"
end
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.equal(res.output.length, 1);
  assert.match(res.output[0], /^head=10\|second=20\|bonus=15\|grade=B\|total=8\|cnt=3\|rsum=4\|caught=true$/);
  // Ensure no false W003 unused warning for loop bounds
  assert.ok(!res.warnings.some((w) => w.code === "W003"));
});

test("supports multiline expressions, pipe |>, nullish ??, stack traces, and modern stdlib helpers", () => {
  const code = `
keep items = [
    "  alpha  ",
    "  beta  "
]
keep piped = items[0] |> trim |> upper
keep cfg = {"mode": "fast"}
keep port = map_opt(cfg, "port") ?? "3000"

keep chunks = list_chunk([1, 2, 3, 4, 5], 2)
keep zipped = list_zip(["a", "b"], [1, 2])
keep re_ok = regex_match("^AL", piped)
keep re_all = regex_find_all("[0-9]+", "v1 and v23")
keep re_rep = regex_replace("[0-9]+", "id-42", "X")
keep g = gcd(24, 36)
keep l = lcm(4, 6)
keep c = clamp(15, 0, 10)
keep sv = semver_compare("1.0.0", "0.3.0")
keep ck = http_cookie_build("sid", "xyz", {"http_only": "true", "path": "/"})
keep ckp = http_cookie_parse("sid=xyz; theme=light")
keep auth = http_auth_bearer("Bearer tok-123")
keep sch = schema_validate({"name": "dog", "ver": 1}, {"name": "text", "ver": "number"})

show $"piped={piped}|port={port}|chunks={len(chunks)}|zip={len(zipped)}|re={re_ok}|re_all={join(re_all, ",")}|re_rep={re_rep}|g={g}|l={l}|c={c}|sv={sv}|auth={auth}|sid={map_get(ckp, "sid")}|ck={contains(ck, "HttpOnly")}|sch={map_get(sch, "valid")}"
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.equal(
    res.output[0],
    "piped=ALPHA|port=3000|chunks=3|zip=2|re=true|re_all=1,23|re_rep=id-X|g=12|l=12|c=10|sv=1|auth=tok-123|sid=xyz|ck=true|sch=true"
  );

  // Verify stack trace on nested runtime error
  const traceRes = runDogSandbox(`
do inner()
    give number("bad-num")
end
do outer()
    give inner()
end
show outer()
`);
  assert.equal(traceRes.ok, false);
  assert.ok(Array.isArray(traceRes.stackFrames) && traceRes.stackFrames.length >= 2);
  assert.match(traceRes.error, /STACK TRACE:/);
  assert.match(traceRes.error, /at inner/);
  assert.match(traceRes.error, /at outer/);
});

test("supports full cross-style interoperability (English + Classic + Simple together)", () => {
  const code = `
define eng_calc(base: number, bonus: number = 4) returns number
    make total = base
    increase total by bonus
    add 2 to total
    give total
end

fn cls_one_liner(x: number) -> number = x * 2
define eng_one_liner(x: number) returns number = x + 3
do simp_one_liner(x) = x - 1

var acc = eng_calc(10)
keep doubled = cls_one_liner(acc)
keep shifted = eng_one_liner(doubled)
make final_val = simp_one_liner(shifted)

if final_val >= 30 then
    bump final_val by 5
or when final_val >= 20
    increase final_val by 2
elif final_val >= 10
    final_val += 1
else if final_val >= 0
    add 1 to final_val
otherwise
    set final_val to 0
end

for a in 1..2
    increase final_val by a
end
each b from 1 to 2
    final_val += b
end
loop 2 times
    bump final_val
end

keep label = when final_val >= 40 then "HIGH" else "LOW"
keep piped = final_val |> do(v) => v + 10 |> fn(v) => v * 2
check piped == (final_val + 10) * 2
show $"val={final_val}|label={label}|piped={piped}"
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  // acc = 10+4+2 = 16; doubled = 32; shifted = 35; final_val = 34; +5=39; +3=42; +3=45; +2=47
  // piped = (47+10)*2 = 114
  assert.deepEqual(res.output, ["val=47|label=HIGH|piped=114"]);

  const mutKeep = runDogSandbox("keep locked = 5\nincrease locked by 1\n");
  assert.equal(mutKeep.ok, false);
  assert.equal(mutKeep.errorCode, "E012");

  const mutLetBump = runDogSandbox("let fixed = 5\nbump fixed\n");
  assert.equal(mutLetBump.ok, false);
  assert.equal(mutLetBump.errorCode, "E012");
});
