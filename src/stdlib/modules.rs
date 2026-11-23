use crate::diagnostics::suggest_similar;

#[derive(Debug, Clone, Copy)]
pub struct StdModuleDef {
    pub name: &'static str,
    pub summary: &'static str,
    /// Pairs of `(member_name, canonical_builtin_name)`
    pub exports: &'static [(&'static str, &'static str)],
}

pub const STD_MODULES: &[StdModuleDef] = &[
    StdModuleDef {
        name: "std.io",
        summary: "Terminal output, interactive prompts, menus, progress, and tables",
        exports: &[
            ("say", "say"),
            ("print", "print"),
            ("answer", "answer"),
            ("ask", "ask"),
            ("confirm", "confirm"),
            ("choose", "choose"),
            ("password", "password"),
            ("read_key", "read_key"),
            ("clear", "clear"),
            ("progress", "progress"),
            ("table", "table"),
            ("log_info", "log_info"),
            ("log_warn", "log_warn"),
            ("log_error", "log_error"),
        ],
    },
    StdModuleDef {
        name: "std.text",
        summary: "UTF-8 string transformation, searching, padding, splitting, and escaping",
        exports: &[
            ("lower", "lower"),
            ("upper", "upper"),
            ("trim", "trim"),
            ("contains", "contains"),
            ("starts_with", "starts_with"),
            ("ends_with", "ends_with"),
            ("replace", "replace"),
            ("slice", "slice"),
            ("split", "split"),
            ("join", "join"),
            ("index_of", "index_of"),
            ("last_index_of", "last_index_of"),
            ("count_matches", "count_matches"),
            ("char_at", "char_at"),
            ("remove_prefix", "remove_prefix"),
            ("remove_suffix", "remove_suffix"),
            ("title_case", "title_case"),
            ("reverse", "reverse_text"),
            ("reverse_text", "reverse_text"),
            ("lines", "lines"),
            ("repeat", "str_repeat"),
            ("str_repeat", "str_repeat"),
            ("pad_left", "pad_left"),
            ("pad_right", "pad_right"),
            ("glob_match", "glob_match"),
            ("html_escape", "html_escape"),
            ("len", "len"),
        ],
    },
    StdModuleDef {
        name: "std.math",
        summary: "Integer & decimal math, trigonometry, logarithms, constants, and RNG",
        exports: &[
            ("abs", "abs"),
            ("min", "min"),
            ("max", "max"),
            ("clamp", "clamp"),
            ("round", "round"),
            ("floor", "floor"),
            ("ceil", "ceil"),
            ("sqrt", "sqrt"),
            ("pow", "pow"),
            ("pow_int", "pow_int"),
            ("sin", "sin"),
            ("cos", "cos"),
            ("tan", "tan"),
            ("atan2", "atan2"),
            ("exp", "exp"),
            ("log", "log"),
            ("log10", "log10"),
            ("sign", "sign"),
            ("gcd", "gcd"),
            ("lcm", "lcm"),
            ("pi", "math_pi"),
            ("e", "math_e"),
            ("math_pi", "math_pi"),
            ("math_e", "math_e"),
            ("rand_int", "rand_int"),
        ],
    },
    StdModuleDef {
        name: "std.files",
        summary: "Filesystem I/O, directory traversal, and cross-platform path helpers",
        exports: &[
            ("read", "file_read"),
            ("write", "file_write"),
            ("append", "file_append"),
            ("exists", "file_exists"),
            ("delete", "file_delete"),
            ("copy", "file_copy"),
            ("move", "file_move"),
            ("size", "file_size"),
            ("is_file", "is_file"),
            ("is_dir", "is_dir"),
            ("list", "dir_list"),
            ("mkdir", "mkdir"),
            ("rmdir", "rmdir"),
            ("join", "path_join"),
            ("dirname", "path_dirname"),
            ("basename", "path_basename"),
            ("ext", "path_ext"),
            ("temp_dir", "temp_dir"),
            ("cwd", "cwd"),
            ("file_read", "file_read"),
            ("file_write", "file_write"),
            ("file_append", "file_append"),
            ("file_exists", "file_exists"),
            ("file_delete", "file_delete"),
            ("file_copy", "file_copy"),
            ("file_move", "file_move"),
            ("file_size", "file_size"),
            ("dir_list", "dir_list"),
            ("path_join", "path_join"),
            ("path_dirname", "path_dirname"),
            ("path_basename", "path_basename"),
            ("path_ext", "path_ext"),
        ],
    },
    StdModuleDef {
        name: "std.net",
        summary: "HTTP/HTTPS client & server, WebSockets, raw TCP/UDP sockets, DNS, and URLs",
        exports: &[
            ("http_get", "http_get"),
            ("http_post", "http_post"),
            ("http_put", "http_put"),
            ("http_delete", "http_delete"),
            ("http_request", "http_request"),
            ("http_request_timeout", "http_request_timeout"),
            ("http_listen", "http_listen"),
            ("http_port", "http_port"),
            ("http_accept", "http_accept"),
            ("http_try_accept", "http_try_accept"),
            ("http_reply", "http_reply"),
            ("http_reply_headers", "http_reply_headers"),
            ("http_redirect", "http_redirect"),
            ("http_serve_file", "http_serve_file"),
            ("http_close", "http_close"),
            ("http_route_match", "http_route_match"),
            ("http_status_text", "http_status_text"),
            ("ws_upgrade", "ws_upgrade"),
            ("ws_connect", "ws_connect"),
            ("ws_send", "ws_send"),
            ("ws_recv", "ws_recv"),
            ("ws_ping", "ws_ping"),
            ("ws_close", "ws_close"),
            ("tcp_listen", "tcp_listen"),
            ("tcp_port", "tcp_port"),
            ("tcp_accept", "tcp_accept"),
            ("tcp_connect", "tcp_connect"),
            ("tcp_send", "tcp_send"),
            ("tcp_recv", "tcp_recv"),
            ("tcp_set_timeout", "tcp_set_timeout"),
            ("tcp_remote_addr", "tcp_remote_addr"),
            ("tcp_close", "tcp_close"),
            ("udp_bind", "udp_bind"),
            ("udp_send_to", "udp_send_to"),
            ("udp_recv_from", "udp_recv_from"),
            ("udp_close", "udp_close"),
            ("dns_lookup", "dns_lookup"),
            ("port_is_open", "port_is_open"),
            ("local_ip", "local_ip"),
            ("url_encode", "url_encode"),
            ("url_decode", "url_decode"),
            ("url_parse", "url_parse"),
            ("query_parse", "query_parse"),
            ("query_stringify", "query_stringify"),
            ("cookie_parse", "cookie_parse"),
        ],
    },
    StdModuleDef {
        name: "std.system",
        summary: "Environment variables, .env loading, subprocesses, time, OS info, and assertions",
        exports: &[
            ("args", "args"),
            ("env_get", "env_get"),
            ("env_set", "env_set"),
            ("env_load", "env_load"),
            ("exec", "exec_cmd"),
            ("exec_cmd", "exec_cmd"),
            ("os_name", "os_name"),
            ("os_arch", "os_arch"),
            ("time_now_ms", "time_now_ms"),
            ("time_utc", "time_utc"),
            ("time_format", "time_format"),
            ("sleep_ms", "sleep_ms"),
            ("uuid_v4", "uuid_v4"),
            ("assert", "assert"),
            ("fail", "fail"),
        ],
    },
    StdModuleDef {
        name: "std.data",
        summary: "JSON, TOML, CSV parsing/serialization and persistent KV storage",
        exports: &[
            ("json_parse", "json_parse"),
            ("json_stringify", "json_stringify"),
            ("json_pretty", "json_pretty"),
            ("json_has_key", "json_has_key"),
            ("json_read", "json_read"),
            ("json_write", "json_write"),
            ("toml_parse", "toml_parse"),
            ("csv_parse", "csv_parse"),
            ("csv_stringify", "csv_stringify"),
            ("kv_set", "kv_set"),
            ("kv_get", "kv_get"),
            ("kv_has", "kv_has"),
            ("kv_delete", "kv_delete"),
            ("kv_keys", "kv_keys"),
        ],
    },
    StdModuleDef {
        name: "std.crypto",
        summary: "SHA-1/256/512, HMAC-SHA256, Hex, Base64/Base64URL, and CSPRNG tokens",
        exports: &[
            ("hex_encode", "hex_encode"),
            ("hex_decode", "hex_decode"),
            ("base64_encode", "base64_encode"),
            ("base64_decode", "base64_decode"),
            ("base64_url_encode", "base64_url_encode"),
            ("base64_url_decode", "base64_url_decode"),
            ("sha1", "sha1"),
            ("sha256", "sha256"),
            ("sha512", "sha512"),
            ("hmac_sha256", "hmac_sha256"),
            ("hmac_sha256_verify", "hmac_sha256_verify"),
            ("rand_bytes_hex", "rand_bytes_hex"),
        ],
    },
    StdModuleDef {
        name: "std.cli",
        summary: "Themed CLI messaging, spinners, progress bars, and terminal capability checks",
        exports: &[
            ("info", "cli_info"),
            ("success", "cli_success"),
            ("warn", "cli_warn"),
            ("error", "cli_error"),
            ("debug", "cli_debug"),
            ("progress", "cli_progress"),
            ("spinner", "cli_spinner"),
            ("theme", "cli_theme"),
            ("prefix", "cli_prefix"),
            ("timestamps", "cli_timestamps"),
            ("supports_color", "cli_supports_color"),
            ("is_tty", "cli_is_tty"),
            ("cli_info", "cli_info"),
            ("cli_success", "cli_success"),
            ("cli_warn", "cli_warn"),
            ("cli_error", "cli_error"),
            ("cli_debug", "cli_debug"),
            ("cli_progress", "cli_progress"),
            ("cli_spinner", "cli_spinner"),
            ("cli_theme", "cli_theme"),
            ("cli_prefix", "cli_prefix"),
            ("cli_timestamps", "cli_timestamps"),
            ("cli_supports_color", "cli_supports_color"),
            ("cli_is_tty", "cli_is_tty"),
        ],
    },
    StdModuleDef {
        name: "std.collections",
        summary: "Mutable lists, sets/dedup, slices, aggregates, and typed maps",
        exports: &[
            ("push", "push"),
            ("pop", "pop"),
            ("insert_at", "insert_at"),
            ("remove_at", "remove_at"),
            ("clear", "clear"),
            ("sort", "sort"),
            ("reverse", "reverse"),
            ("list_contains", "list_contains"),
            ("list_index_of", "list_index_of"),
            ("list_slice", "list_slice"),
            ("list_unique", "list_unique"),
            ("list_take", "list_take"),
            ("list_skip", "list_skip"),
            ("list_flatten", "list_flatten"),
            ("list_sum", "list_sum"),
            ("list_avg", "list_avg"),
            ("list_concat", "list_concat"),
            ("range", "range"),
            ("map_get", "map_get"),
            ("map_get_or", "map_get_or"),
            ("map_set", "map_set"),
            ("map_has", "map_has"),
            ("map_keys", "map_keys"),
            ("map_values", "map_values"),
            ("map_remove", "map_remove"),
            ("map_clear", "map_clear"),
            ("map_merge", "map_merge"),
            ("map_from_lists", "map_from_lists"),
            ("len", "len"),
        ],
    },
    StdModuleDef {
        name: "std.css",
        summary: "DOG Style Sheets (.dgss DSL), variables, shorthands, responsive media, and keyframes",
        exports: &[
            ("dog_css", "dog_css"),
            ("compile", "dog_css"),
            ("rule", "css_rule"),
            ("css_rule", "css_rule"),
            ("vars", "css_vars"),
            ("css_vars", "css_vars"),
            ("media", "css_media"),
            ("css_media", "css_media"),
            ("keyframes", "css_keyframes"),
            ("css_keyframes", "css_keyframes"),
            ("theme", "css_theme"),
            ("css_theme", "css_theme"),
            ("bundle", "css_bundle"),
            ("css_bundle", "css_bundle"),
        ],
    },
    StdModuleDef {
        name: "std.web",
        summary: "Client-side interactive DOM APIs, event listeners, storage, HTML & UI components",
        exports: &[
            ("html_el", "html_el"),
            ("el", "html_el"),
            ("html_page", "html_page"),
            ("page", "html_page"),
            ("nav", "web_nav"),
            ("web_nav", "web_nav"),
            ("card", "web_card"),
            ("web_card", "web_card"),
            ("grid", "web_grid"),
            ("web_grid", "web_grid"),
            ("button", "web_button"),
            ("web_button", "web_button"),
            ("input", "web_input"),
            ("web_input", "web_input"),
            ("mount", "dom_mount"),
            ("dom_mount", "dom_mount"),
            ("set_text", "dom_set_text"),
            ("dom_set_text", "dom_set_text"),
            ("set_html", "dom_set_html"),
            ("dom_set_html", "dom_set_html"),
            ("get_value", "dom_get_value"),
            ("dom_get_value", "dom_get_value"),
            ("set_value", "dom_set_value"),
            ("dom_set_value", "dom_set_value"),
            ("add_class", "dom_add_class"),
            ("dom_add_class", "dom_add_class"),
            ("remove_class", "dom_remove_class"),
            ("dom_remove_class", "dom_remove_class"),
            ("toggle_class", "dom_toggle_class"),
            ("dom_toggle_class", "dom_toggle_class"),
            ("set_style", "dom_set_style"),
            ("dom_set_style", "dom_set_style"),
            ("on", "dom_on"),
            ("dom_on", "dom_on"),
            ("title", "web_title"),
            ("web_title", "web_title"),
            ("storage_set", "web_storage_set"),
            ("web_storage_set", "web_storage_set"),
            ("storage_get", "web_storage_get"),
            ("web_storage_get", "web_storage_get"),
            ("route", "web_route"),
            ("web_route", "web_route"),
            ("navigate", "web_navigate"),
            ("web_navigate", "web_navigate"),
        ],
    },
];

pub fn find_std_module(name: &str) -> Option<&'static StdModuleDef> {
    STD_MODULES.iter().find(|m| m.name == name)
}

pub fn is_std_module(name: &str) -> bool {
    find_std_module(name).is_some()
}

pub fn std_module_names() -> Vec<&'static str> {
    STD_MODULES.iter().map(|m| m.name).collect()
}

pub fn suggest_std_module(target: &str) -> Option<&'static str> {
    suggest_similar(target, STD_MODULES.iter().map(|m| m.name))
}

pub fn resolve_std_call(module: &str, func: &str) -> Result<&'static str, String> {
    let Some(def) = find_std_module(module) else {
        let mut msg = format!("Unknown standard library module '{module}'.");
        if let Some(hint) = suggest_std_module(module) {
            msg.push_str(&format!(" Did you mean '{hint}'?"));
        }
        return Err(msg);
    };
    if let Some((_, canonical)) = def.exports.iter().find(|(member, _)| *member == func) {
        return Ok(*canonical);
    }
    let mut msg = format!("Module '{module}' has no function '{func}()'.");
    if let Some(hint) = suggest_similar(func, def.exports.iter().map(|(m, _)| *m)) {
        msg.push_str(&format!(" Did you mean '{module}.{hint}()'?"));
    }
    Err(msg)
}

pub fn render_std_docs(topic: Option<&str>) -> String {
    if let Some(name) = topic {
        if let Some(def) = find_std_module(name) {
            let mut out = format!("DOG Standard Module: {}\n", def.name);
            out.push_str(&format!("{}\n", "-".repeat(21 + def.name.len())));
            out.push_str(&format!("{}\n\nExports:\n", def.summary));
            for (member, canonical) in def.exports {
                let sig = crate::editor::builtin_signature_help(canonical).unwrap_or("");
                if member == canonical {
                    out.push_str(&format!("  {module}.{member}  — {sig}\n", module = def.name));
                } else {
                    out.push_str(&format!(
                        "  {module}.{member} (alias: {canonical})  — {sig}\n",
                        module = def.name
                    ));
                }
            }
            return out;
        }
    }

    let mut out = String::from(
        "DOG Standard Library Modules (v0.3.0)\n\
         -------------------------------------\n\
         Import with `use \"std.io\"` or call directly via `std.<module>.<fn>(...)`.\n\
         All flat global built-ins remain available without prefixes.\n\n",
    );
    for m in STD_MODULES {
        out.push_str(&format!("  {:<17} {}\n", m.name, m.summary));
    }
    out.push_str("\nInspect any module with e.g. `dog docs std.io` or `dog docs std.net`.");
    out
}
