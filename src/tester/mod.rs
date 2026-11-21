use crate::config::Personality;
use crate::interpreter::run_source_with_io;
use crate::stdlib::InterpreterIo;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct CaptureIo {
    output: String,
}

impl CaptureIo {
    fn new() -> Self {
        Self {
            output: String::new(),
        }
    }
}

impl InterpreterIo for CaptureIo {
    fn write_stdout(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn writeln_stdout(&mut self, text: &str) {
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn read_line(&mut self) -> Result<String, String> {
        Err("Interactive answer() is not available during non-interactive `dog test`.".to_string())
    }
}

pub struct TestReport {
    pub passed: usize,
    pub failed: usize,
}

pub fn run_tests(target: Option<&str>, personality: Personality) -> Result<TestReport, String> {
    let files = collect_test_files(target)?;
    if files.is_empty() {
        return Err("No test files (*_test.dg) found.".to_string());
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let suite_start = Instant::now();

    for path in &files {
        let label = path.display().to_string();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("FAIL {label}: could not read file ({err})");
                failed += 1;
                continue;
            }
        };

        let mut io = CaptureIo::new();
        let start = Instant::now();
        match run_source_with_io(&label, &source, &mut io) {
            Ok(()) => {
                let ms = start.elapsed().as_millis();
                println!("PASS {label} ({ms} ms)");
                passed += 1;
            }
            Err(diag) => {
                eprintln!("FAIL {label}");
                eprintln!("{}", diag.format_with(personality, Some(&source)));
                failed += 1;
            }
        }
    }

    let total_ms = suite_start.elapsed().as_millis();
    println!("\nTest summary: {passed} passed, {failed} failed ({total_ms} ms)");
    Ok(TestReport { passed, failed })
}

fn collect_test_files(target: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let base = match target {
        Some(p) => PathBuf::from(p),
        None => {
            if Path::new("tests").is_dir() {
                PathBuf::from("tests")
            } else {
                PathBuf::from(".")
            }
        }
    };

    if base.is_file() {
        return Ok(vec![base]);
    }
    if !base.exists() {
        return Err(format!("Test path '{}' does not exist.", base.display()));
    }

    let mut out = Vec::new();
    walk_dir(&base, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read '{}': {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
            }
            walk_dir(&path, out)?;
        } else if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.ends_with("_test.dg") {
                out.push(path);
            }
        }
    }
    Ok(())
}
