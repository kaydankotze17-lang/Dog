use crate::config::DogConfig;
use crate::sdu::detect_actual_root;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

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
    let has_rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

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
    println!("Sessions run    : {}", cfg.runs_completed);
    println!("Working dir     : {cwd}");
    println!(
        "Native rustc    : {}",
        if has_rustc {
            "available (`dog build --native` supported)"
        } else {
            "not found (`dog build` emits portable .dgb bundles)"
        }
    );

    let tty_status = if cfg!(unix) {
        "unix tty supported (stty)"
    } else {
        "non-unix terminal"
    };
    println!("Editor/Studio   : {tty_status}");
    println!(
        "Project pack    : {}",
        if Path::new("dog.pack").is_file() {
            "dog.pack present"
        } else {
            "none in current directory"
        }
    );

    if Path::new("examples/hello.dg").is_file() {
        match fs::read_to_string("examples/hello.dg") {
            Ok(_) => println!("Sample check    : examples/hello.dg readable"),
            Err(e) => println!("Sample check    : examples/hello.dg error ({e})"),
        }
    }
}
