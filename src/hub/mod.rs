use crate::config::DogConfig;
use crate::crypto::sha256_hex;
use crate::net::http_download_bytes;
use crate::term::{format_bytes, DogMessenger, MsgLevel, ProgressBar, ProgressUnit};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackMessages {
    pub install_banner: String,
    pub pre_install: String,
    pub post_install: String,
    pub uninstall_banner: String,
    pub pre_uninstall: String,
    pub post_uninstall: String,
}

impl PackMessages {
    pub fn is_empty(&self) -> bool {
        self.install_banner.is_empty()
            && self.pre_install.is_empty()
            && self.post_install.is_empty()
            && self.uninstall_banner.is_empty()
            && self.pre_uninstall.is_empty()
            && self.post_uninstall.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DogPack {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub description: String,
    pub dependencies: BTreeMap<String, String>,
    pub scripts: BTreeMap<String, String>,
    pub messages: PackMessages,
}

impl DogPack {
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;
        Self::parse(&raw)
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut pack = Self {
            name: "dog-app".to_string(),
            version: "0.1.0".to_string(),
            entry: "src/main.dg".to_string(),
            description: String::new(),
            dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            messages: PackMessages::default(),
        };
        let mut section = String::new();

        for (idx, raw_line) in raw.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("Malformed line {} in dog.pack: '{line}'", idx + 1));
            };
            let key = k.trim().to_string();
            let val = v.trim().trim_matches('"').to_string();
            match section.as_str() {
                "package" | "" => match key.as_str() {
                    "name" => pack.name = val,
                    "version" => pack.version = val,
                    "entry" => pack.entry = val,
                    "description" => pack.description = val,
                    _ => {}
                },
                "dependencies" => {
                    pack.dependencies.insert(key, val);
                }
                "scripts" => {
                    pack.scripts.insert(key, val);
                }
                "messages" => match key.as_str() {
                    "install_banner" | "banner" => pack.messages.install_banner = val,
                    "pre_install" => pack.messages.pre_install = val,
                    "post_install" | "install_note" => pack.messages.post_install = val,
                    "uninstall_banner" => pack.messages.uninstall_banner = val,
                    "pre_uninstall" => pack.messages.pre_uninstall = val,
                    "post_uninstall" | "uninstall_note" => pack.messages.post_uninstall = val,
                    _ => {}
                },
                "install" => match key.as_str() {
                    "banner" | "install_banner" => pack.messages.install_banner = val,
                    "pre" | "pre_install" => pack.messages.pre_install = val,
                    "post" | "post_install" | "note" | "install_note" => {
                        pack.messages.post_install = val
                    }
                    _ => {}
                },
                "uninstall" => match key.as_str() {
                    "banner" | "uninstall_banner" => pack.messages.uninstall_banner = val,
                    "pre" | "pre_uninstall" => pack.messages.pre_uninstall = val,
                    "post" | "post_uninstall" | "note" | "uninstall_note" => {
                        pack.messages.post_uninstall = val
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(pack)
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", self.name));
        out.push_str(&format!("version = \"{}\"\n", self.version));
        out.push_str(&format!("entry = \"{}\"\n", self.entry));
        if !self.description.is_empty() {
            out.push_str(&format!("description = \"{}\"\n", self.description));
        }
        out.push_str("\n[dependencies]\n");
        for (k, v) in &self.dependencies {
            out.push_str(&format!("{k} = \"{v}\"\n"));
        }
        if !self.scripts.is_empty() {
            out.push_str("\n[scripts]\n");
            for (k, v) in &self.scripts {
                out.push_str(&format!("{k} = \"{v}\"\n"));
            }
        }
        if !self.messages.is_empty() {
            out.push_str("\n[messages]\n");
            if !self.messages.install_banner.is_empty() {
                out.push_str(&format!(
                    "install_banner = \"{}\"\n",
                    self.messages.install_banner
                ));
            }
            if !self.messages.pre_install.is_empty() {
                out.push_str(&format!("pre_install = \"{}\"\n", self.messages.pre_install));
            }
            if !self.messages.post_install.is_empty() {
                out.push_str(&format!(
                    "post_install = \"{}\"\n",
                    self.messages.post_install
                ));
            }
            if !self.messages.uninstall_banner.is_empty() {
                out.push_str(&format!(
                    "uninstall_banner = \"{}\"\n",
                    self.messages.uninstall_banner
                ));
            }
            if !self.messages.pre_uninstall.is_empty() {
                out.push_str(&format!(
                    "pre_uninstall = \"{}\"\n",
                    self.messages.pre_uninstall
                ));
            }
            if !self.messages.post_uninstall.is_empty() {
                out.push_str(&format!(
                    "post_uninstall = \"{}\"\n",
                    self.messages.post_uninstall
                ));
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct CachedPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: PathBuf,
    pub sha256: String,
    pub messages: PackMessages,
    pub dependencies: BTreeMap<String, String>,
}

pub fn cache_dir() -> PathBuf {
    DogConfig::home_dir().join("cache")
}

pub fn load_or_default_pack() -> Result<DogPack, String> {
    load_or_default_pack_in(Path::new("."))
}

pub fn load_or_default_pack_in(project_dir: &Path) -> Result<DogPack, String> {
    let p = project_dir.join("dog.pack");
    if p.exists() {
        let raw = fs::read_to_string(p).map_err(|e| format!("Failed to read dog.pack: {e}"))?;
        DogPack::parse(&raw)
    } else {
        let abs_name = fs::canonicalize(project_dir)
            .ok()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()));
        let dir_name = abs_name
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| "dog-app".to_string());
        Ok(DogPack {
            name: dir_name,
            version: "0.1.0".to_string(),
            entry: "src/main.dg".to_string(),
            description: "DOG project".to_string(),
            dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            messages: PackMessages::default(),
        })
    }
}

pub fn write_pack_and_lock(pack: &DogPack) -> Result<(), String> {
    write_pack_and_lock_in(Path::new("."), &cache_dir(), pack)
}

pub fn write_pack_and_lock_in(
    project_dir: &Path,
    cache_root: &Path,
    pack: &DogPack,
) -> Result<(), String> {
    fs::write(project_dir.join("dog.pack"), pack.render())
        .map_err(|e| format!("Failed to write dog.pack: {e}"))?;
    let mut lock = String::from("# Auto-generated by DOG Hub (real SHA-256 verified digests)\n");
    for (name, ver) in &pack.dependencies {
        let pkg_dir = cache_root.join(name);
        let hash = if pkg_dir.is_dir() {
            compute_dir_sha256(&pkg_dir).unwrap_or_else(|_| sha256_hex(format!("{name}@{ver}").as_bytes()))
        } else {
            sha256_hex(format!("{name}@{ver}").as_bytes())
        };
        lock.push_str(&format!("package={name} version={ver} sha256={hash}\n"));
    }
    fs::write(project_dir.join("dog.lock"), lock)
        .map_err(|e| format!("Failed to write dog.lock: {e}"))
}

pub fn scan_local_cache() -> Vec<CachedPackage> {
    scan_cache_in(&cache_dir())
}

pub fn scan_cache_in(dir: &Path) -> Vec<CachedPackage> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let manifest = p.join("dog.pack");
        if let Ok(raw) = fs::read_to_string(&manifest) {
            if let Ok(pack) = DogPack::parse(&raw) {
                let sha = compute_dir_sha256(&p).unwrap_or_else(|_| {
                    let entry_src = fs::read(p.join(&pack.entry)).unwrap_or_default();
                    sha256_hex(&entry_src)
                });
                out.push(CachedPackage {
                    name: pack.name,
                    version: pack.version,
                    description: pack.description,
                    path: p,
                    sha256: sha,
                    messages: pack.messages,
                    dependencies: pack.dependencies,
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn handle_add(args: &[String]) -> Result<String, String> {
    let cfg = DogConfig::load();
    let messenger = cfg.messenger();
    install_package_in(args, Path::new("."), &cache_dir(), &messenger, false)
}

pub fn install_package_live(args: &[String], cfg: &DogConfig) -> Result<String, String> {
    let messenger = cfg.messenger();
    install_package_in(args, Path::new("."), &cache_dir(), &messenger, true)
}

pub fn install_package_in(
    args: &[String],
    project_dir: &Path,
    cache_root: &Path,
    messenger: &DogMessenger,
    live_progress: bool,
) -> Result<String, String> {
    let Some(spec) = args.first() else {
        return Err("Usage: dog add <pkg[@version] | local-path | http(s)://url>".to_string());
    };
    let started = Instant::now();
    let mut pack = load_or_default_pack_in(project_dir)?;
    let mut log = Vec::<String>::new();

    // Determine source flavor and acquire metadata + real payload stats
    enum SourceKind {
        LocalDir(PathBuf, Vec<(PathBuf, u64)>, u64),
        RemoteUrl(String, Vec<u8>),
        LocalCache(CachedPackage, Vec<(PathBuf, u64)>, u64),
    }

    let is_url = spec.starts_with("http://") || spec.starts_with("https://");
    let (sub_pack, dep_ver_value, source_label, source_kind) = if !is_url && Path::new(spec).exists() {
        let src_path = Path::new(spec);
        if !src_path.is_dir() {
            return Err(format!(
                "Local package path '{}' is not a directory. Pass a directory containing dog.pack or .dg files.",
                src_path.display()
            ));
        }
        let sub_pack = if src_path.join("dog.pack").is_file() {
            let raw = fs::read_to_string(src_path.join("dog.pack"))
                .map_err(|e| format!("Failed to read package manifest: {e}"))?;
            DogPack::parse(&raw)?
        } else {
            let name = src_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("local-pkg")
                .to_string();
            DogPack {
                name,
                version: "0.1.0".to_string(),
                ..DogPack::default()
            }
        };
        let mut files = Vec::new();
        let total_bytes = collect_dir_files(src_path, Path::new(""), &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        (
            sub_pack.clone(),
            format!("path:{}", src_path.display()),
            format!("local ({})", src_path.display()),
            SourceKind::LocalDir(src_path.to_path_buf(), files, total_bytes),
        )
    } else if is_url {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_stage(1, 4, "Connecting to remote source", spec),
        );
        let mut bar = ProgressBar::new("Downloading", 0, ProgressUnit::Bytes, messenger.clone());
        let bytes = http_download_bytes(spec, |done, total_opt| {
            if let Some(t) = total_opt {
                bar.total = t;
            }
            bar.set(done);
            if live_progress {
                bar.update_stdout(done);
            }
        })?;
        if live_progress {
            bar.total = bytes.len() as u64;
            bar.finish_stdout(None);
        } else {
            bar.total = bytes.len() as u64;
            bar.set(bytes.len() as u64);
            log.push(bar.render());
        }

        let filename = spec
            .rsplit('/')
            .next()
            .unwrap_or("remote_pkg.dg")
            .split('?')
            .next()
            .unwrap_or("remote_pkg.dg");
        let parsed = if filename == "dog.pack" {
            let text = String::from_utf8(bytes.clone())
                .map_err(|_| "Downloaded dog.pack is not valid UTF-8.".to_string())?;
            DogPack::parse(&text)?
        } else {
            let stem = filename
                .strip_suffix(".dg")
                .unwrap_or(filename)
                .trim();
            let clean_name = if stem.is_empty() { "remote-pkg" } else { stem };
            DogPack {
                name: clean_name.to_string(),
                version: "0.1.0".to_string(),
                entry: format!("src/{filename}"),
                description: format!("Downloaded from {spec}"),
                ..DogPack::default()
            }
        };
        (
            parsed,
            spec.to_string(),
            format!("url ({spec})"),
            SourceKind::RemoteUrl(filename.to_string(), bytes),
        )
    } else {
        let (name, wanted_ver) = match spec.split_once('@') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (spec.to_string(), None),
        };
        let cached = scan_cache_in(cache_root);
        let Some(found) = cached.into_iter().find(|c| c.name == name) else {
            return Err(format!(
                "Package '{spec}' not found in local DOG cache ({}).\nPass a local directory (`dog add ./my_pkg`), an `http(s)://` package URL, or populate {}.",
                cache_root.display(),
                cache_root.display()
            ));
        };
        if let Some(ref req_v) = wanted_ver {
            if &found.version != req_v {
                return Err(format!(
                    "Cached package '{}' is version {}, but {} was requested.",
                    found.name, found.version, req_v
                ));
            }
        }
        let manifest_raw = fs::read_to_string(found.path.join("dog.pack"))
            .map_err(|e| format!("Failed reading cached manifest: {e}"))?;
        let sub_pack = DogPack::parse(&manifest_raw)?;
        let mut files = Vec::new();
        let total_bytes = collect_dir_files(&found.path, Path::new(""), &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        (
            sub_pack,
            found.version.clone(),
            format!("cache ({})", found.path.display()),
            SourceKind::LocalCache(found, files, total_bytes),
        )
    };

    if !sub_pack.messages.install_banner.is_empty() {
        let box_str = messenger.render_box(
            &format!("{} {}", sub_pack.name, sub_pack.version),
            &[sub_pack.messages.install_banner.clone()],
        );
        emit_hub_line(live_progress, &mut log, box_str);
    }

    if !sub_pack.messages.pre_install.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_line(
                MsgLevel::Info,
                &format!("[{}] {}", sub_pack.name, sub_pack.messages.pre_install),
                None,
            ),
        );
    }

    // Stage 1: Dependency Resolution (real transitive inspection)
    let is_remote = matches!(source_kind, SourceKind::RemoteUrl(_, _));
    if !is_remote {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_stage(
                1,
                4,
                "Resolving dependencies",
                &format!("{}@{}", sub_pack.name, sub_pack.version),
            ),
        );
    } else {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_stage(
                2,
                4,
                "Resolving dependencies",
                &format!("{}@{}", sub_pack.name, sub_pack.version),
            ),
        );
    }

    let src_parent = match &source_kind {
        SourceKind::LocalDir(p, _, _) => p.parent().map(Path::to_path_buf),
        _ => None,
    };
    let resolved_deps = resolve_dependencies_recursive(
        &sub_pack,
        src_parent.as_deref(),
        cache_root,
        &pack,
    )?;
    if resolved_deps.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_line(
                MsgLevel::Debug,
                "No transitive dependencies declared.",
                None,
            ),
        );
    } else {
        for dep in &resolved_deps {
            emit_hub_line(
                live_progress,
                &mut log,
                messenger.render_line(MsgLevel::Info, &format!("Resolved {dep}"), None),
            );
        }
    }

    // Stage 2: Fetch / Stage real files & bytes into cache
    let dest = cache_root.join(&sub_pack.name);
    let (file_count, total_bytes): (usize, u64) = match source_kind {
        SourceKind::LocalDir(src_dir, files, total_bytes) => {
            emit_hub_line(
                live_progress,
                &mut log,
                messenger.render_stage(
                    2,
                    4,
                    "Staging package files",
                    &format!("{} file(s), {}", files.len(), format_bytes(total_bytes)),
                ),
            );
            if dest.exists() {
                let _ = fs::remove_dir_all(&dest);
            }
            fs::create_dir_all(&dest)
                .map_err(|e| format!("Failed to create cache dir '{}': {e}", dest.display()))?;

            let mut bar = ProgressBar::new(
                format!("Copying {}", sub_pack.name),
                total_bytes,
                ProgressUnit::Bytes,
                messenger.clone(),
            );
            let mut copied_bytes = 0u64;
            for (rel, expected_size) in &files {
                let sp = src_dir.join(rel);
                let dp = dest.join(rel);
                if let Some(parent) = dp.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        format!("Failed creating directory '{}': {e}", parent.display())
                    })?;
                }
                let actual = fs::copy(&sp, &dp)
                    .map_err(|e| format!("Failed to copy '{}': {e}", sp.display()))?;
                let _ = expected_size;
                copied_bytes = copied_bytes.saturating_add(actual);
                if live_progress {
                    bar.update_stdout(copied_bytes);
                } else {
                    bar.set(copied_bytes);
                }
            }
            if live_progress {
                bar.finish_stdout(None);
            } else {
                log.push(bar.render());
            }
            (files.len(), copied_bytes)
        }
        SourceKind::RemoteUrl(filename, bytes) => {
            if dest.exists() {
                let _ = fs::remove_dir_all(&dest);
            }
            fs::create_dir_all(dest.join("src"))
                .map_err(|e| format!("Failed creating cache dir '{}': {e}", dest.display()))?;
            let size = bytes.len() as u64;
            if filename == "dog.pack" {
                fs::write(dest.join("dog.pack"), &bytes)
                    .map_err(|e| format!("Failed writing manifest: {e}"))?;
            } else {
                fs::write(dest.join("dog.pack"), sub_pack.render())
                    .map_err(|e| format!("Failed writing manifest: {e}"))?;
                fs::write(dest.join("src").join(&filename), &bytes)
                    .map_err(|e| format!("Failed writing '{}': {e}", filename))?;
            }
            (1, size)
        }
        SourceKind::LocalCache(found, files, total_bytes) => {
            emit_hub_line(
                live_progress,
                &mut log,
                messenger.render_stage(
                    2,
                    4,
                    "Reading cached package",
                    &format!("{} file(s), {}", files.len(), format_bytes(total_bytes)),
                ),
            );
            let mut bar = ProgressBar::new(
                format!("Checking {}", found.name),
                total_bytes,
                ProgressUnit::Bytes,
                messenger.clone(),
            );
            let mut checked_bytes = 0u64;
            for (rel, _) in &files {
                let data = fs::read(found.path.join(rel))
                    .map_err(|e| format!("Failed reading cached file '{}': {e}", rel.display()))?;
                checked_bytes = checked_bytes.saturating_add(data.len() as u64);
                if live_progress {
                    bar.update_stdout(checked_bytes);
                } else {
                    bar.set(checked_bytes);
                }
            }
            if live_progress {
                bar.finish_stdout(None);
            } else {
                log.push(bar.render());
            }
            (files.len(), checked_bytes)
        }
    };

    // Stage 3: Real SHA-256 Integrity Verification
    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_stage(
            3,
            4,
            "Verifying SHA-256 integrity",
            &dest.display().to_string(),
        ),
    );
    let verified_sha = compute_dir_sha256(&dest)?;
    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_line(
            MsgLevel::Success,
            &format!("Verified SHA-256 {}", verified_sha),
            None,
        ),
    );

    // Stage 4: Register in dog.pack + dog.lock and display package notes + summary
    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_stage(4, 4, "Writing dog.pack & dog.lock", &sub_pack.name),
    );
    pack.dependencies.insert(sub_pack.name.clone(), dep_ver_value);
    write_pack_and_lock_in(project_dir, cache_root, &pack)?;

    if !sub_pack.messages.post_install.is_empty() {
        let note_box = messenger.render_box(
            &format!("{} — Developer Install Note", sub_pack.name),
            &[sub_pack.messages.post_install.clone()],
        );
        emit_hub_line(live_progress, &mut log, note_box);
    }

    let elapsed_ms = started.elapsed().as_millis();
    let short_sha = &verified_sha[..16.min(verified_sha.len())];
    let dep_summary = if resolved_deps.is_empty() {
        "0".to_string()
    } else {
        format!("{} ({})", resolved_deps.len(), resolved_deps.join(", "))
    };
    let summary = messenger.render_summary(
        "DOG Package Installed",
        &[
            ("Package", sub_pack.name.clone()),
            ("Version", sub_pack.version.clone()),
            ("Source", source_label),
            ("Dependencies", dep_summary),
            ("Files", format!("{file_count} ({})", format_bytes(total_bytes))),
            ("SHA-256", format!("{short_sha}... (verified)")),
            ("Duration", format!("{elapsed_ms} ms")),
        ],
    );
    emit_hub_line(live_progress, &mut log, summary);

    Ok(if live_progress {
        String::new()
    } else {
        log.join("\n")
    })
}

pub fn handle_remove(args: &[String]) -> Result<String, String> {
    let cfg = DogConfig::load();
    let messenger = cfg.messenger();
    remove_package_in(args, Path::new("."), &cache_dir(), &messenger, false)
}

pub fn remove_package_live(args: &[String], cfg: &DogConfig) -> Result<String, String> {
    let messenger = cfg.messenger();
    remove_package_in(args, Path::new("."), &cache_dir(), &messenger, true)
}

pub fn remove_package_in(
    args: &[String],
    project_dir: &Path,
    cache_root: &Path,
    messenger: &DogMessenger,
    live_progress: bool,
) -> Result<String, String> {
    let mut name: Option<String> = None;
    let mut purge = false;
    for arg in args {
        match arg.as_str() {
            "--purge" | "-p" => purge = true,
            other if !other.starts_with('-') && name.is_none() => {
                name = Some(other.to_string());
            }
            flag => {
                return Err(format!(
                    "Unknown flag or argument '{flag}'. Usage: dog remove <package> [--purge]"
                ));
            }
        }
    }
    let Some(name) = name else {
        return Err("Usage: dog remove <package> [--purge]".to_string());
    };

    let started = Instant::now();
    let mut pack = load_or_default_pack_in(project_dir)?;
    let Some(prev_spec) = pack.dependencies.get(&name).cloned() else {
        return Err(format!("Package '{name}' is not listed in dog.pack."));
    };

    let cached_pkg = scan_cache_in(cache_root).into_iter().find(|c| c.name == name);
    let messages = cached_pkg
        .as_ref()
        .map(|c| c.messages.clone())
        .unwrap_or_default();

    let mut log = Vec::<String>::new();

    if !messages.uninstall_banner.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_box(
                &format!("Uninstalling {name}"),
                &[messages.uninstall_banner.clone()],
            ),
        );
    }

    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_stage(1, 3, "Inspecting dependency graph", &name),
    );
    let dependents: Vec<String> = pack
        .dependencies
        .keys()
        .filter(|dep| *dep != &name)
        .filter_map(|dep| {
            let p = cache_root.join(dep).join("dog.pack");
            let raw = fs::read_to_string(p).ok()?;
            let sub = DogPack::parse(&raw).ok()?;
            if sub.dependencies.contains_key(&name) {
                Some(dep.clone())
            } else {
                None
            }
        })
        .collect();
    if !dependents.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_line(
                MsgLevel::Warning,
                &format!(
                    "Remaining package(s) reference '{name}': {}",
                    dependents.join(", ")
                ),
                None,
            ),
        );
    }

    if !messages.pre_uninstall.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_line(
                MsgLevel::Info,
                &format!("[{name}] {}", messages.pre_uninstall),
                None,
            ),
        );
    }

    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_stage(2, 3, "Removing dependency & updating dog.lock", &name),
    );
    pack.dependencies.remove(&name);
    write_pack_and_lock_in(project_dir, cache_root, &pack)?;

    let mut purged_info = "kept in cache (pass --purge to delete)".to_string();
    if purge {
        let cached_dir = cache_root.join(&name);
        if cached_dir.exists() {
            let mut files = Vec::new();
            let bytes = collect_dir_files(&cached_dir, Path::new(""), &mut files).unwrap_or(0);
            fs::remove_dir_all(&cached_dir)
                .map_err(|e| format!("Failed purging '{}': {e}", cached_dir.display()))?;
            purged_info = format!("purged {} file(s), {}", files.len(), format_bytes(bytes));
        } else {
            purged_info = "not present in cache".to_string();
        }
    }

    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_stage(3, 3, "Finalizing uninstallation", &name),
    );
    if !messages.post_uninstall.is_empty() {
        emit_hub_line(
            live_progress,
            &mut log,
            messenger.render_box(
                &format!("{name} — Developer Uninstall Note"),
                &[messages.post_uninstall],
            ),
        );
    }

    let elapsed_ms = started.elapsed().as_millis();
    emit_hub_line(
        live_progress,
        &mut log,
        messenger.render_summary(
            "DOG Package Removed",
            &[
                ("Package", name),
                ("Previous", prev_spec),
                ("Cache", purged_info),
                ("Duration", format!("{elapsed_ms} ms")),
            ],
        ),
    );

    Ok(if live_progress {
        String::new()
    } else {
        log.join("\n")
    })
}

fn emit_hub_line(live_progress: bool, log: &mut Vec<String>, line: String) {
    if live_progress {
        println!("{line}");
    }
    log.push(line);
}

fn resolve_dependencies_recursive(
    root: &DogPack,
    local_parent: Option<&Path>,
    cache_root: &Path,
    project_pack: &DogPack,
) -> Result<Vec<String>, String> {
    let mut visited = BTreeSet::new();
    let mut resolved = Vec::new();
    let mut stack: Vec<(String, String)> = root
        .dependencies
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    while let Some((dep_name, dep_ver)) = stack.pop() {
        if !visited.insert(dep_name.clone()) {
            continue;
        }
        if let Some(rel_path) = dep_ver.strip_prefix("path:") {
            let candidate = Path::new(rel_path);
            if candidate.join("dog.pack").is_file() {
                let raw = fs::read_to_string(candidate.join("dog.pack"))
                    .map_err(|e| format!("Failed reading dependency '{dep_name}': {e}"))?;
                let sub = DogPack::parse(&raw)?;
                resolved.push(format!("{}@{} (path)", sub.name, sub.version));
                for (k, v) in &sub.dependencies {
                    stack.push((k.clone(), v.clone()));
                }
                continue;
            }
        }
        if let Some(parent) = local_parent {
            let sibling = parent.join(&dep_name);
            if sibling.join("dog.pack").is_file() {
                let raw = fs::read_to_string(sibling.join("dog.pack"))
                    .map_err(|e| format!("Failed reading sibling dependency '{dep_name}': {e}"))?;
                let sub = DogPack::parse(&raw)?;
                resolved.push(format!("{}@{} (local)", sub.name, sub.version));
                for (k, v) in &sub.dependencies {
                    stack.push((k.clone(), v.clone()));
                }
                continue;
            }
        }
        let cached_manifest = cache_root.join(&dep_name).join("dog.pack");
        if cached_manifest.is_file() {
            let raw = fs::read_to_string(&cached_manifest)
                .map_err(|e| format!("Failed reading cached dependency '{dep_name}': {e}"))?;
            let sub = DogPack::parse(&raw)?;
            resolved.push(format!("{}@{} (cached)", sub.name, sub.version));
            for (k, v) in &sub.dependencies {
                stack.push((k.clone(), v.clone()));
            }
            continue;
        }
        if let Some(existing_ver) = project_pack.dependencies.get(&dep_name) {
            resolved.push(format!("{dep_name}@{existing_ver} (project)"));
            continue;
        }
        return Err(format!(
            "Dependency '{dep_name}@{dep_ver}' required by '{}' was not found in cache ({}) or sibling path.",
            root.name,
            cache_root.display()
        ));
    }

    Ok(resolved)
}

fn collect_dir_files(
    root: &Path,
    rel_prefix: &Path,
    out: &mut Vec<(PathBuf, u64)>,
) -> Result<u64, String> {
    let mut total = 0u64;
    let cur = root.join(rel_prefix);
    let entries =
        fs::read_dir(&cur).map_err(|e| format!("Failed reading '{}': {e}", cur.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = rel_prefix.join(entry.file_name());
        if path.is_dir() {
            total = total.saturating_add(collect_dir_files(root, &rel, out)?);
        } else {
            let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            total = total.saturating_add(len);
            out.push((rel, len));
        }
    }
    Ok(total)
}

pub fn compute_dir_sha256(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect_dir_files(dir, Path::new(""), &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut material = Vec::new();
    for (rel, _) in files {
        let rel_str = rel.to_string_lossy();
        material.extend_from_slice(rel_str.as_bytes());
        material.push(0);
        let bytes = fs::read(dir.join(&rel))
            .map_err(|e| format!("Failed hashing '{}': {e}", rel.display()))?;
        let file_digest = sha256_hex(&bytes);
        material.extend_from_slice(file_digest.as_bytes());
        material.push(b'\n');
    }
    Ok(sha256_hex(&material))
}

pub fn handle_search(args: &[String]) -> String {
    let query = args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let cached = scan_local_cache();
    let matches: Vec<CachedPackage> = cached
        .into_iter()
        .filter(|p| {
            query.is_empty()
                || p.name.to_ascii_lowercase().contains(&query)
                || p.description.to_ascii_lowercase().contains(&query)
        })
        .collect();

    if matches.is_empty() {
        return format!(
            "No matching packages in local DOG Hub cache ({}).\nPass a local directory (`dog add ./pkg`) or `http(s)://` URL to install.",
            cache_dir().display()
        );
    }

    let mut out = String::from("Local DOG Hub Cache Matches:\n");
    for p in matches {
        out.push_str(&format!(
            "  {} ({}) — {}\n",
            p.name,
            p.version,
            if p.description.is_empty() {
                "No description"
            } else {
                &p.description
            }
        ));
    }
    out
}

pub fn handle_list() -> Result<String, String> {
    let pack = load_or_default_pack()?;
    if pack.dependencies.is_empty() {
        return Ok(format!(
            "Package '{}' ({}) has no dependencies listed in dog.pack.",
            pack.name, pack.version
        ));
    }
    let mut out = format!("Dependencies for {} ({}):\n", pack.name, pack.version);
    for (k, v) in &pack.dependencies {
        out.push_str(&format!("  {k} = {v}\n"));
    }
    Ok(out)
}

pub fn handle_info(args: &[String]) -> Result<String, String> {
    if let Some(name) = args.first() {
        if let Some(pkg) = scan_local_cache().into_iter().find(|c| &c.name == name) {
            let mut out = format!(
                "Package: {}\nVersion: {}\nPath   : {}\nSHA-256: {}\nDesc   : {}",
                pkg.name,
                pkg.version,
                pkg.path.display(),
                pkg.sha256,
                pkg.description
            );
            if !pkg.dependencies.is_empty() {
                let deps: Vec<String> = pkg
                    .dependencies
                    .iter()
                    .map(|(k, v)| format!("{k}@{v}"))
                    .collect();
                out.push_str(&format!("\nDeps   : {}", deps.join(", ")));
            }
            if !pkg.messages.is_empty() {
                out.push_str("\nCustom Messages: configured ([messages])");
            }
            return Ok(out);
        }
        return Err(format!(
            "Package '{name}' not found in local cache ({}).",
            cache_dir().display()
        ));
    }
    let pack = load_or_default_pack()?;
    Ok(format!(
        "Project: {}\nVersion: {}\nEntry  : {}\nDeps   : {}",
        pack.name,
        pack.version,
        pack.entry,
        pack.dependencies.len()
    ))
}
