use crate::config::DogConfig;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn handle_sdu(args: &[String]) -> Result<String, String> {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    match sub {
        "status" => Ok(status_report()),
        "auth" => authorize_session(args.get(1..).unwrap_or(&[])),
        "permissions" => Ok(permissions_report()),
        "revoke" => revoke_session(),
        "logs" => read_audit_logs(),
        "doctor" => Ok(sdu_doctor_report()),
        "help" | "--help" | "-h" => Ok(sdu_help()),
        other => Err(format!(
            "Unknown SDU command '{other}'. Run `dog sdu help` for valid subcommands."
        )),
    }
}

pub fn session_path() -> PathBuf {
    DogConfig::home_dir().join("sdu_session")
}

pub fn audit_log_path() -> PathBuf {
    DogConfig::home_dir().join("logs").join("sdu_audit.log")
}

pub fn detect_actual_root() -> bool {
    #[cfg(unix)]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("Uid:") {
                    if rest.split_whitespace().nth(1) == Some("0") {
                        return true;
                    }
                }
            }
        }
        false
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn tool_exists(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {cmd} >/dev/null 2>&1"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_session_expiry() -> Option<u64> {
    let raw = fs::read_to_string(session_path()).ok()?;
    for line in raw.lines() {
        if let Some(val) = line.strip_prefix("expires_at=") {
            if let Ok(ts) = val.trim().parse::<u64>() {
                return Some(ts);
            }
        }
    }
    None
}

fn active_session_remaining() -> Option<u64> {
    let exp = read_session_expiry()?;
    let now = now_secs();
    if exp > now {
        Some(exp - now)
    } else {
        None
    }
}

fn append_audit(action: &str, detail: &str) {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}] action={} detail={}", now_secs(), action, detail);
    }
}

fn status_report() -> String {
    let actual_root = detect_actual_root();
    let is_termux = env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    let has_sudo = tool_exists("sudo");
    let has_su = tool_exists("su");
    let has_rish = tool_exists("rish");
    let session_str = match active_session_remaining() {
        Some(rem) => format!("active (expires in {rem}s)"),
        None => "inactive".to_string(),
    };

    format!(
        "SDU (Super Dog User) Status\n\
         ---------------------------\n\
         Actual kernel EUID=0 : {}\n\
         Termux environment   : {}\n\
         OS tools available   : sudo={}, su={}, shizuku(rish)={}\n\
         SDU scoped session   : {}\n\
         Audit log            : {}",
        if actual_root { "yes (real root)" } else { "no (standard non-root)" },
        if is_termux { "yes" } else { "no" },
        has_sudo,
        has_su,
        has_rish,
        session_str,
        audit_log_path().display()
    )
}

fn authorize_session(args: &[String]) -> Result<String, String> {
    let ttl_secs: u64 = args
        .first()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(900)
        .min(3600);

    let confirmed = args.iter().any(|a| a == "--confirm");
    if !confirmed {
        return Err(
            "Explicit authorization required: run `dog sdu auth --confirm` (optional TTL in seconds first, max 3600).\n\
             Note: SDU sessions authorize scoped DOG operations only and never fake OS root privileges."
                .to_string(),
        );
    }

    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create ~/.dog directory: {e}"))?;
    }
    let exp = now_secs() + ttl_secs;
    let body = format!(
        "authorized=true\ncreated_at={}\nexpires_at={}\nactual_root={}\n",
        now_secs(),
        exp,
        detect_actual_root()
    );
    fs::write(&path, body).map_err(|e| format!("Failed to write SDU session: {e}"))?;
    append_audit("sdu.auth", &format!("ttl={ttl_secs}s"));

    Ok(format!(
        "SDU scoped session authorized for {ttl_secs}s (audit entry recorded)."
    ))
}

fn permissions_report() -> String {
    let active = active_session_remaining().is_some();
    let real_root = detect_actual_root();
    format!(
        "SDU Capability Matrix\n\
         ---------------------\n\
         Read/Check/Format .dg : always permitted (standard user)\n\
         Local HTTP/Test/Docs  : always permitted (standard user)\n\
         SDU Scoped Operations : {}\n\
         Kernel Root Actions   : {}",
        if active {
            "permitted (explicit session active)"
        } else {
            "locked (run `dog sdu auth --confirm`)"
        },
        if real_root {
            "permitted by OS (real EUID=0)"
        } else {
            "unavailable (non-root user; requires real OS sudo/su/Shizuku)"
        }
    )
}

fn revoke_session() -> Result<String, String> {
    let path = session_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to revoke SDU session: {e}"))?;
        append_audit("sdu.revoke", "session_removed");
        Ok("SDU session revoked.".to_string())
    } else {
        Ok("No active SDU session to revoke.".to_string())
    }
}

fn read_audit_logs() -> Result<String, String> {
    let path = audit_log_path();
    if !path.exists() {
        return Ok(format!("No SDU audit logs recorded yet at {}.", path.display()));
    }
    fs::read_to_string(&path).map_err(|e| format!("Failed to read SDU audit log: {e}"))
}

fn sdu_doctor_report() -> String {
    let mut lines = vec![status_report()];
    if !detect_actual_root() {
        lines.push(
            "\nPrivilege Diagnosis: Running as non-root. DOG never fabricates root access.\n\
             For system-level operations, invoke DOG under `sudo` (Linux) or `tsu`/`rish` (Termux/Shizuku) if needed."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn sdu_help() -> String {
    "SDU (Super Dog User) Subcommands:\n\
     \n\
       dog sdu status             Show actual OS privilege status and SDU session\n\
       dog sdu auth --confirm     Create a temporary scoped SDU session (default 900s)\n\
       dog sdu permissions        Show active SDU capability matrix\n\
       dog sdu revoke             Revoke current SDU session immediately\n\
       dog sdu logs               Display ~/.dog/logs/sdu_audit.log\n\
       dog sdu doctor             Diagnose privilege & Termux/Shizuku availability"
        .to_string()
}
