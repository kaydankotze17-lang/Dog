use crate::sdu::detect_actual_root;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const START_MARKER: &str = "# >>> DOG SHELL PROMPT >>>";
const END_MARKER: &str = "# <<< DOG SHELL PROMPT <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

impl ShellKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }

    pub fn default_rc_path(&self) -> PathBuf {
        let home = user_home_dir();
        match self {
            Self::Bash => home.join(".bashrc"),
            Self::Zsh => home.join(".zshrc"),
            Self::Fish => home.join(".config").join("fish").join("config.fish"),
        }
    }
}

pub fn user_home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn detect_user_shell() -> ShellKind {
    if let Ok(sh) = env::var("SHELL") {
        let lower = sh.to_ascii_lowercase();
        if lower.contains("zsh") {
            return ShellKind::Zsh;
        }
        if lower.contains("fish") {
            return ShellKind::Fish;
        }
    }
    ShellKind::Bash
}

pub fn detect_git_status_tag(dir: &Path) -> Option<String> {
    let branch_out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !branch_out.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
    if branch.is_empty() {
        return None;
    }
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    if dirty {
        Some(format!("{branch}*"))
    } else {
        Some(branch)
    }
}

pub fn render_live_prompt() -> String {
    let user = env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .unwrap_or_else(|_| {
            if env::var("PREFIX").is_ok_and(|p| p.contains("com.termux")) {
                "termux".to_string()
            } else {
                "nonroot".to_string()
            }
        });
    let (display_user, sigil) = if detect_actual_root() {
        ("root".to_string(), '#')
    } else {
        (user, '$')
    };

    let cwd_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("~"));
    let cwd = cwd_path.display().to_string();
    let display_dir = if let Ok(home) = env::var("HOME") {
        if let Some(stripped) = cwd.strip_prefix(&home) {
            if stripped.is_empty() {
                "~".to_string()
            } else {
                format!("~{stripped}")
            }
        } else {
            cwd
        }
    } else {
        cwd
    };

    let blue = "\x1b[1;34m";
    let red = "\x1b[1;31m";
    let green = "\x1b[1;32m";
    let yellow = "\x1b[1;33m";
    let reset = "\x1b[0m";

    let git_seg = match detect_git_status_tag(&cwd_path) {
        Some(tag) => format!("{blue}-[{yellow}{tag}{blue}]"),
        None => String::new(),
    };

    format!(
        "{blue}┌──({red}{display_user}⩫dog{blue})-[{green}{display_dir}{blue}]{git_seg}\n└─{red}{sigil}{reset} "
    )
}

pub fn handle_prompt(args: &[String]) -> Result<String, String> {
    let sub = args.first().map(String::as_str).unwrap_or("preview");
    let rest = args.get(1..).unwrap_or(&[]);

    match sub {
        "preview" => Ok(preview_output()),
        "init" | "export" => {
            let shell = match rest.first().map(String::as_str) {
                Some(s) => ShellKind::parse(s).ok_or_else(|| {
                    format!("Unknown shell '{s}'. Supported shells: bash, zsh, fish.")
                })?,
                None => detect_user_shell(),
            };
            let include_banner = !rest.iter().any(|a| a == "--no-banner");
            Ok(generate_shell_snippet(shell, include_banner))
        }
        "install" => install_prompt(rest),
        "uninstall" | "remove" => uninstall_prompt(rest),
        "status" => Ok(status_report()),
        "help" | "--help" | "-h" => Ok(prompt_help()),
        other => Err(format!(
            "Unknown prompt subcommand '{other}'. Run `dog prompt help`."
        )),
    }
}

pub fn handle_completions(shell_arg: Option<&str>) -> Result<String, String> {
    let shell = match shell_arg {
        Some(s) => ShellKind::parse(s).ok_or_else(|| {
            format!("Unknown shell '{s}'. Supported shells: bash, zsh, fish.")
        })?,
        None => detect_user_shell(),
    };
    Ok(generate_completion_snippet(shell))
}

fn preview_output() -> String {
    format!(
        "DOG Terminal Prompt Preview:\n\n{}echo \"DOG ready!\"\n\n\
         Install into your terminal (Termux / Linux / macOS):\n\
           dog prompt install              # Auto-detect bash/zsh/fish and install\n\
           dog prompt install --no-banner  # Prompt only (no login greeting)\n\
           dog prompt uninstall            # Cleanly remove DOG prompt\n\
           dog prompt status               # Check installed RC files",
        render_live_prompt()
    )
}

pub fn generate_completion_snippet(shell: ShellKind) -> String {
    let cmds = "run eval build package check lint bench benchmark format fmt test coverage debug watch graph explain inspect profile repl playground edit studio forge new add remove search list info prompt completions docs learn server doctor system status env process permissions logs clean update upgrade config sdu version help";
    match shell {
        ShellKind::Bash => format!(
            "_dog_complete() {{\n\
                 local cur prev\n\
                 cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n\
                 prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"\n\
                 if [[ $COMP_CWORD -eq 1 ]]; then\n\
                     COMPREPLY=( $(compgen -W \"{cmds}\" -- \"$cur\") )\n\
                     return 0\n\
                 fi\n\
                 case \"$prev\" in\n\
                     prompt) COMPREPLY=( $(compgen -W \"preview install uninstall status init\" -- \"$cur\") ) ;;\n\
                     sdu) COMPREPLY=( $(compgen -W \"status auth permissions revoke logs doctor\" -- \"$cur\") ) ;;\n\
                     docs) COMPREPLY=( $(compgen -W \"std syntax english types stdlib network cli errors editor hub sdu prompt\" -- \"$cur\") ) ;;\n\
                     learn) COMPREPLY=( $(compgen -W \"basics english input functions data errors\" -- \"$cur\") ) ;;\n\
                     *) COMPREPLY=( $(compgen -f -- \"$cur\") ) ;;\n\
                 esac\n\
             }}\n\
             complete -F _dog_complete dog\n"
        ),
        ShellKind::Zsh => format!(
            "#compdef dog\n\
             _dog_complete() {{\n\
                 local -a cmds\n\
                 cmds=({cmds})\n\
                 if (( CURRENT == 2 )); then\n\
                     _describe 'command' cmds\n\
                 else\n\
                     _files\n\
                 fi\n\
             }}\n\
             if type compdef >/dev/null 2>&1; then compdef _dog_complete dog; fi\n"
        ),
        ShellKind::Fish => {
            let mut out = String::new();
            for cmd in cmds.split_whitespace() {
                out.push_str(&format!(
                    "complete -c dog -n '__fish_use_subcommand' -a '{cmd}'\n"
                ));
            }
            out
        }
    }
}

pub fn generate_shell_snippet(shell: ShellKind, include_banner: bool) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match shell {
        ShellKind::Bash => {
            let banner_block = if include_banner {
                format!(
                    "if [[ $- == *i* ]] && [[ -z \"${{__DOG_BANNER_SHOWN:-}}\" ]]; then\n\
                         export __DOG_BANNER_SHOWN=1\n\
                         printf '\\033[1;34m[DOG {version}]\\033[0m Termux/Shell ready — \\033[36mdog studio .\\033[0m | \\033[36mdog edit\\033[0m | \\033[36mdog repl\\033[0m\\n'\n\
                     fi\n"
                )
            } else {
                String::new()
            };
            format!(
                "{START_MARKER}\n\
                 __dog_prompt_cmd() {{\n\
                     local rc=$?\n\
                     local u=\"${{USER:-$(id -un 2>/dev/null || echo shell)}}\"\n\
                     local sig='$'\n\
                     if [[ ${{EUID:-$(id -u 2>/dev/null || echo 1000)}} -eq 0 ]]; then\n\
                         u='root'\n\
                         sig='#'\n\
                     fi\n\
                     local d=\"$PWD\"\n\
                     if [[ -n \"${{HOME:-}}\" && \"$d\" == \"$HOME\"* ]]; then\n\
                         d=\"~${{d#$HOME}}\"\n\
                     fi\n\
                     local g=''\n\
                     if command -v git >/dev/null 2>&1; then\n\
                         local br\n\
                         br=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD 2>/dev/null)\n\
                         if [[ -n \"$br\" ]]; then\n\
                             local dirty=''\n\
                             [[ -n \"$(git status --porcelain --untracked-files=no 2>/dev/null)\" ]] && dirty='*'\n\
                             g=\"\\[\\e[1;34m\\]-[\\[\\e[1;33m\\]${{br}}${{dirty}}\\[\\e[1;34m\\]]\"\n\
                         fi\n\
                     fi\n\
                     local err=''\n\
                     if [[ $rc -ne 0 ]]; then\n\
                         err=\"\\[\\e[1;34m\\]-[\\[\\e[1;31m\\]✗ $rc\\[\\e[1;34m\\]]\"\n\
                     fi\n\
                     PS1=\"\\[\\e[1;34m\\]┌──(\\[\\e[1;31m\\]${{u}}⩫dog\\[\\e[1;34m\\])-[\\[\\e[1;32m\\]${{d}}\\[\\e[1;34m\\]]${{g}}${{err}}\\n\\[\\e[1;34m\\]└─\\[\\e[1;31m\\]${{sig}}\\[\\e[0m\\] \"\n\
                 }}\n\
                 PROMPT_COMMAND=__dog_prompt_cmd\n\
                 {}\
                 {banner_block}\
                 {END_MARKER}\n",
                generate_completion_snippet(ShellKind::Bash)
            )
        }
        ShellKind::Zsh => {
            let banner_block = if include_banner {
                format!(
                    "if [[ -o interactive ]] && [[ -z \"${{__DOG_BANNER_SHOWN:-}}\" ]]; then\n\
                         export __DOG_BANNER_SHOWN=1\n\
                         print -P '%F{{blue}}%B[DOG {version}]%b%f Shell ready — %F{{cyan}}dog studio .%f | %F{{cyan}}dog edit%f | %F{{cyan}}dog repl%f'\n\
                     fi\n"
                )
            } else {
                String::new()
            };
            format!(
                "{START_MARKER}\n\
                 setopt PROMPT_SUBST\n\
                 __dog_zsh_precmd() {{\n\
                     local rc=$?\n\
                     local u=\"%n\"\n\
                     local sig='%#'\n\
                     local g=''\n\
                     if command -v git >/dev/null 2>&1; then\n\
                         local br\n\
                         br=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD 2>/dev/null)\n\
                         if [[ -n \"$br\" ]]; then\n\
                             local dirty=''\n\
                             [[ -n \"$(git status --porcelain --untracked-files=no 2>/dev/null)\" ]] && dirty='*'\n\
                             g=\"%F{{blue}}-[%F{{yellow}}${{br}}${{dirty}}%F{{blue}}]\"\n\
                         fi\n\
                     fi\n\
                     local err=''\n\
                     if [[ $rc -ne 0 ]]; then\n\
                         err=\"%F{{blue}}-[%F{{red}}✗ $rc%F{{blue}}]\"\n\
                     fi\n\
                     PROMPT=\"%B%F{{blue}}┌──(%F{{red}}${{u}}⩫dog%F{{blue}})-[%F{{green}}%~%F{{blue}}]${{g}}${{err}}$'\\n'└─%F{{red}}${{sig}}%f%b \"\n\
                 }}\n\
                 autoload -Uz add-zsh-hook && add-zsh-hook precmd __dog_zsh_precmd\n\
                 {}\
                 {banner_block}\
                 {END_MARKER}\n",
                generate_completion_snippet(ShellKind::Zsh)
            )
        }
        ShellKind::Fish => {
            let banner_block = if include_banner {
                format!(
                    "if status is-interactive; and not set -q __DOG_BANNER_SHOWN\n\
                         set -gx __DOG_BANNER_SHOWN 1\n\
                         echo (set_color -o blue)\"[DOG {version}]\"(set_color normal)\" Shell ready — dog studio . | dog edit | dog repl\"\n\
                     end\n"
                )
            } else {
                String::new()
            };
            format!(
                "{START_MARKER}\n\
                 function fish_prompt\n\
                     set -l rc $status\n\
                     set -l u (whoami)\n\
                     set -l sig '$'\n\
                     if test (id -u) -eq 0\n\
                         set u 'root'\n\
                         set sig '#'\n\
                     end\n\
                     set -l g ''\n\
                     if type -q git\n\
                         set -l br (git symbolic-ref --short HEAD 2>/dev/null; or git rev-parse --short HEAD 2>/dev/null)\n\
                         if test -n \"$br\"\n\
                             set g (set_color -o blue)\"-[\"(set_color -o yellow)\"$br\"(set_color -o blue)\"]\"\n\
                         end\n\
                     end\n\
                     set -l err ''\n\
                     if test $rc -ne 0\n\
                         set err (set_color -o blue)\"-[\"(set_color -o red)\"✗ $rc\"(set_color -o blue)\"]\"\n\
                     end\n\
                     echo -s (set_color -o blue) \"┌──(\" (set_color -o red) \"$u⩫dog\" (set_color -o blue) \")-[\" (set_color -o green) (prompt_pwd) (set_color -o blue) \"]\" $g $err\n\
                     echo -n -s (set_color -o blue) \"└─\" (set_color -o red) \"$sig\" (set_color normal) \" \"\n\
                 end\n\
                 {}\
                 {banner_block}\
                 {END_MARKER}\n",
                generate_completion_snippet(ShellKind::Fish)
            )
        }
    }
}

fn install_prompt(args: &[String]) -> Result<String, String> {
    let mut shell_opt: Option<ShellKind> = None;
    let mut rc_override: Option<PathBuf> = None;
    let mut include_banner = true;
    let mut install_all = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--no-banner" => include_banner = false,
            "--all" => install_all = true,
            "--shell" | "-s" => {
                let Some(val) = args.get(i + 1) else {
                    return Err("Missing shell name after --shell (bash|zsh|fish|all).".to_string());
                };
                if val.eq_ignore_ascii_case("all") {
                    install_all = true;
                } else {
                    shell_opt = Some(ShellKind::parse(val).ok_or_else(|| {
                        format!("Invalid shell '{val}'. Use bash, zsh, fish, or all.")
                    })?);
                }
                i += 1;
            }
            "--rc" => {
                let Some(val) = args.get(i + 1) else {
                    return Err("Missing file path after --rc.".to_string());
                };
                rc_override = Some(PathBuf::from(val));
                i += 1;
            }
            other => {
                return Err(format!("Unexpected argument '{other}' in `dog prompt install`."));
            }
        }
        i += 1;
    }

    if let Some(rc_path) = rc_override {
        let shell = shell_opt.unwrap_or_else(detect_user_shell);
        let snippet = generate_shell_snippet(shell, include_banner);
        upsert_fenced_block(&rc_path, &snippet)?;
        return Ok(format!(
            "Installed DOG terminal prompt ({}) into {}.\nActivate now with: source {}",
            shell.as_str(),
            rc_path.display(),
            rc_path.display()
        ));
    }

    let targets: Vec<(ShellKind, PathBuf)> = if install_all {
        vec![
            (ShellKind::Bash, ShellKind::Bash.default_rc_path()),
            (ShellKind::Zsh, ShellKind::Zsh.default_rc_path()),
            (ShellKind::Fish, ShellKind::Fish.default_rc_path()),
        ]
    } else if let Some(sh) = shell_opt {
        vec![(sh, sh.default_rc_path())]
    } else {
        let detected = detect_user_shell();
        vec![(detected, detected.default_rc_path())]
    };

    let mut updated = Vec::new();
    for (shell, path) in &targets {
        let snippet = generate_shell_snippet(*shell, include_banner);
        upsert_fenced_block(path, &snippet)?;
        updated.push(format!("{} ({})", path.display(), shell.as_str()));
    }

    let activate_cmd = match targets.first() {
        Some((ShellKind::Fish, p)) => format!("source {}", p.display()),
        Some((_, p)) => format!("source {}", p.display()),
        None => "exec $SHELL".to_string(),
    };

    Ok(format!(
        "Installed DOG terminal prompt:\n  - {}\n\nActivate immediately with:\n  {activate_cmd}",
        updated.join("\n  - ")
    ))
}

fn uninstall_prompt(args: &[String]) -> Result<String, String> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--rc" {
            let Some(val) = args.get(i + 1) else {
                return Err("Missing file path after --rc.".to_string());
            };
            paths.push(PathBuf::from(val));
            i += 1;
        }
        i += 1;
    }
    if paths.is_empty() {
        paths = vec![
            ShellKind::Bash.default_rc_path(),
            ShellKind::Zsh.default_rc_path(),
            ShellKind::Fish.default_rc_path(),
        ];
    }

    let mut removed = Vec::new();
    for path in &paths {
        if remove_fenced_block(path)? {
            removed.push(path.display().to_string());
        }
    }

    if removed.is_empty() {
        Ok("No DOG terminal prompt block was found in checked shell config files.".to_string())
    } else {
        Ok(format!(
            "Removed DOG terminal prompt from:\n  - {}\nRestart your shell or run `exec $SHELL` to restore your previous prompt.",
            removed.join("\n  - ")
        ))
    }
}

fn status_report() -> String {
    let detected = detect_user_shell();
    let is_termux = env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
    let rc_files = [
        ShellKind::Bash.default_rc_path(),
        ShellKind::Zsh.default_rc_path(),
        ShellKind::Fish.default_rc_path(),
    ];

    let mut lines = vec![
        "DOG Shell Prompt Status".to_string(),
        "-----------------------".to_string(),
        format!("Detected shell : {}", detected.as_str()),
        format!("Termux env     : {}", if is_termux { "yes" } else { "no" }),
    ];
    for p in &rc_files {
        let state = match fs::read_to_string(p) {
            Ok(raw) if raw.contains(START_MARKER) => "installed",
            Ok(_) => "not installed",
            Err(_) => "no rc file",
        };
        lines.push(format!("{:<14} : {} ({})", p.file_name().and_then(|s| s.to_str()).unwrap_or("rc"), state, p.display()));
    }
    lines.join("\n")
}

pub fn upsert_fenced_block(path: &Path, block: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory '{}': {e}", parent.display()))?;
        }
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let cleaned = strip_block_text(&existing);
    let mut out = cleaned.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(block);
    fs::write(path, out).map_err(|e| format!("Failed to write '{}': {e}", path.display()))
}

pub fn remove_fenced_block(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;
    if !existing.contains(START_MARKER) {
        return Ok(false);
    }
    let cleaned = strip_block_text(&existing);
    let mut out = cleaned.trim_end().to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    fs::write(path, out).map_err(|e| format!("Failed to write '{}': {e}", path.display()))?;
    Ok(true)
}

fn strip_block_text(raw: &str) -> String {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in raw.lines() {
        if line.trim() == START_MARKER {
            inside = true;
            continue;
        }
        if line.trim() == END_MARKER {
            inside = false;
            continue;
        }
        if !inside {
            kept.push(line);
        }
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn prompt_help() -> String {
    "DOG Terminal Prompt & Shell Integration:\n\
     \n\
       dog prompt                      Show live prompt preview & setup guide\n\
       dog prompt install              Install Kali-style DOG prompt into ~/.bashrc or ~/.zshrc\n\
       dog prompt install --all        Install into bash, zsh, and fish configs\n\
       dog prompt install --no-banner  Skip the interactive shell startup banner\n\
       dog prompt install --rc <file>  Install into a custom shell rc file\n\
       dog prompt uninstall            Remove DOG prompt block cleanly\n\
       dog prompt status               Inspect installed shell configurations\n\
       dog prompt init [bash|zsh|fish] Print raw shell script for eval"
        .to_string()
}
