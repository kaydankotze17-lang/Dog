use crate::config::DogConfig;
use std::env;
use std::fs;
use std::path::Path;

pub fn run_doctor() {
    let version = env!("CARGO_PKG_VERSION");
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let is_termux = env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    let is_root = detect_actual_root();
    let cfg_path = DogConfig::config_path();
    let cfg = DogConfig::load();
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    println!("DOG Doctor Report");
    println!("-----------------");
    println!("CLI version     : dog {version}");
    println!("Target OS/Arch  : {os}/{arch}");
    println!(
        "Termux detected : {}",
        if is_termux { "yes" } else { "no" }
    );
    println!(
        "Privileges      : {}",
        if is_root {
            "root (actual EUID=0)"
        } else {
            "standard non-root user"
        }
    );
    println!("Config file     : {}", cfg_path.display());
    println!("Personality     : {}", cfg.personality.as_str());
    println!("Working dir     : {cwd}");

    let tty_status = if cfg!(unix) {
        "unix tty supported (stty)"
    } else {
        "non-unix terminal"
    };
    println!("Editor TTY      : {tty_status}");

    if Path::new("examples/hello.dg").is_file() {
        match fs::read_to_string("examples/hello.dg") {
            Ok(_) => println!("Sample check    : examples/hello.dg readable"),
            Err(e) => println!("Sample check    : examples/hello.dg error ({e})"),
        }
    }
}

fn detect_actual_root() -> bool {
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
