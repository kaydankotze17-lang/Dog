# DOG Language Roadmap (1,040 Capabilities Across 20 Domains)

Full item-by-item unique-ID catalog: [`docs/CAPABILITIES_CATALOG.md`](CAPABILITIES_CATALOG.md) (`DOG-0001`..`DOG-1040`).

## Status Legend
- `implemented`: Built and active in `src/`
- `tested`: Implemented in `src/` and covered in `tests/`
- `specified`: Syntax, types, and edge cases defined in specification
- `proposed`: Planned capability slot in the 20-domain taxonomy

## Domain Allocations (Total: 1,040 Capabilities)
1. **Variables & Data Types (60)** — `DOG-0001`..`DOG-0060` (`let`, `var`, `const`, strict primitives, `list<T>`, `map<K, V>`, `struct`, `enum` implemented; tuples, slices, unions specified/proposed)
2. **Functions & Execution (70)** — `DOG-0061`..`DOG-0130` (`fn`, recursion depth guard, return-path check, built-ins implemented; closures, default params, generics specified/proposed)
3. **Operators & Expressions (50)** — `DOG-0131`..`DOG-0180` (`== != > < >= <= and or not + - * / % . []` implemented; range/pipe operators specified/proposed)
4. **Control Flow & Loops (50)** — `DOG-0181`..`DOG-0230` (`if`/`otherwise`/`end`, `match`/`case`, `while`, `repeat`, `for`, `break`, `continue` implemented)
5. **Collections & Data Structures (70)** — `DOG-0231`..`DOG-0300` (`list<T>`, `push`, `pop`, `map<K, V>`, `map_get`, `map_set`, `map_has`, `map_keys`, `map_remove` implemented; sets/queues/trees specified/proposed)
6. **Object-Oriented & Composite Modeling (70)** — `DOG-0301`..`DOG-0370` (`struct`, field access/mutation, `enum` variants implemented; methods, traits/interfaces specified/proposed)
7. **Generics & Advanced Typing (60)** — `DOG-0371`..`DOG-0430` (`list<T>`, `map<K, V>`, contextual literal hints, strict type inference implemented; user generic `fn<T>`/`struct<T>` specified/proposed)
8. **Error Handling (40)** — `DOG-0431`..`DOG-0470` (`try ... catch err ... end`, `fail()`, `assert()`, 4-part `E001`..`E041` diagnostics implemented)
9. **Concurrency & Async (60)** — `DOG-0471`..`DOG-0530` (thread sleep/timing implemented; `spawn`, channels, `async`/`await` specified/proposed)
10. **Networking & APIs (70)** — `DOG-0531`..`DOG-0600` (`http_listen`/`http_accept`/`http_reply`/`http_serve_file`/`http_close`, `http_get`/`http_post`/`http_request` with `https://` curl bridge, raw `tcp_*` sockets, URL encode/decode, and multi-threaded `dog server --host` implemented & tested; WebSockets/gRPC specified/proposed)
11. **File & System Operations (60)** — `DOG-0601`..`DOG-0660` (`file_*`, `dir_list`, `mkdir`, `cwd`, `args`, `env_get`, `env_set`, `exec_cmd`, `os_name`, `os_arch`, `uuid_v4`, `time_now_ms`, `dog system/process/env/permissions` implemented)
12. **Database Functionality (50)** — `DOG-0661`..`DOG-0710` (embedded JSON KV database `kv_set`/`kv_get`/`kv_has`/`kv_delete`/`kv_keys` implemented & tested; embedded B-tree & SQLite driver specified/proposed)
13. **Security & Cryptography (50)** — `DOG-0711`..`DOG-0760` (pure-Rust FIPS 180-4 `sha256`, RFC 2104 `hmac_sha256`, `hex_*`, `base64_*`, `dog sdu`, `dog env` masking implemented; AES/Ed25519 specified/proposed)
14. **Testing & Debugging (50)** — `DOG-0761`..`DOG-0810` (`dog test`, `assert()`, `dog inspect`, `dog profile`, `dog doctor`, `dog logs` implemented)
15. **Metaprogramming & Reflection (50)** — `DOG-0811`..`DOG-0860` (`dog inspect` AST outline & symbol reflection implemented; compile-time macros & derive attributes specified/proposed)
16. **Modules & Package Development (40)** — `DOG-0861`..`DOG-0900` (`use "path.dg"` cycle-checked imports, `dog.pack`, `dog.lock`, `dog add/remove/search/list/info`, `dog forge` implemented)
17. **Performance & Memory Management (40)** — `DOG-0901`..`DOG-0940` (`.dgb` bundle compiler, `rustc` `--native` transpiler, `dog profile` implemented; bytecode VM & arena allocator specified/proposed)
18. **Developer Productivity (50)** — `DOG-0941`..`DOG-0990` (`dog edit`, `dog studio`, `dog format`, `dog repl`, `dog prompt`, `dog completions`, `dog docs`, personality diagnostics & tips implemented)
19. **Functional Programming (25)** — `DOG-0991`..`DOG-1015` (pure top-level `fn`, recursion, immutable `let`/`const` default implemented; lambdas, `map`/`filter`/`reduce` specified/proposed)
20. **Serialization & Interoperability (25)** — `DOG-1016`..`DOG-1040` (`json_parse`, `json_stringify`, `csv_parse`, `url_encode`/`url_decode`, `.dgb` binary encoding, `exec_cmd`, `rustc` `--native` implemented; TOML/C-ABI specified/proposed)
