use crate::ast::{Program, Stmt};
use crate::config::Personality;
use crate::interpreter::{
    check_source, run_source_with_io, stmt_span, DebugStepDecision, ExecObserver, Interpreter,
};
use crate::runtime::Environment;
use crate::stdlib::InterpreterIo;
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub file: String,
    pub covered_lines: usize,
    pub total_lines: usize,
    pub percent: f64,
}

#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub total_covered: usize,
    pub total_lines: usize,
    pub percent: f64,
    pub files: Vec<FileCoverage>,
    pub report_text: String,
}

struct CoverageObserver<'a> {
    executed: &'a mut BTreeMap<String, BTreeSet<usize>>,
}

impl<'a> ExecObserver for CoverageObserver<'a> {
    fn on_before_stmt(&mut self, stmt: &Stmt, _env: &Environment) -> DebugStepDecision {
        let span = stmt_span(stmt);
        self.executed
            .entry(span.file.clone())
            .or_default()
            .insert(span.line);
        DebugStepDecision::Continue
    }
}

pub fn run_tests(target: Option<&str>, personality: Personality) -> Result<TestReport, String> {
    let files = collect_test_files(target)?;
    if files.is_empty() {
        return Err("No test files (*_test.dg) found.".to_string());
    }

    let messenger = crate::config::DogConfig::load().messenger();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let suite_start = Instant::now();

    for path in &files {
        let label = path.display().to_string();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(err) => {
                messenger.error(&format!("FAIL {label}: could not read file ({err})"));
                failed += 1;
                continue;
            }
        };

        let mut io = CaptureIo::new();
        let start = Instant::now();
        match run_source_with_io(&label, &source, &mut io) {
            Ok(()) => {
                let ms = start.elapsed().as_millis();
                messenger.success(&format!("PASS {label} ({ms} ms)"));
                passed += 1;
            }
            Err(diag) => {
                messenger.error(&format!("FAIL {label}"));
                if !io.output.is_empty() {
                    eprintln!("--- stdout ---\n{}", io.output.trim_end());
                }
                eprintln!("{}", diag.format_with(personality, Some(&source)));
                failed += 1;
            }
        }
    }

    let total_ms = suite_start.elapsed().as_millis();
    let summary_line = format!("Test summary: {passed} passed, {failed} failed ({total_ms} ms)");
    if failed == 0 {
        messenger.success(&summary_line);
    } else {
        messenger.error(&summary_line);
    }
    Ok(TestReport { passed, failed })
}

pub fn run_coverage(
    target: Option<&str>,
    personality: Personality,
) -> Result<CoverageReport, String> {
    let files = if let Some(p) = target {
        let path = Path::new(p);
        if path.is_file() {
            vec![path.to_path_buf()]
        } else {
            collect_test_files(Some(p))?
        }
    } else {
        collect_test_files(None)?
    };

    if files.is_empty() {
        return Err("No test or .dg files found to measure coverage.".to_string());
    }

    let mut all_lines: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut executed: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut tests_passed = 0usize;
    let mut tests_failed = 0usize;

    for path in &files {
        let label = path.display().to_string();
        let source = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{label}': {e}"))?;
        let program = check_source(&label, &source)
            .map_err(|d| d.format_with(personality, Some(&source)))?;
        collect_program_lines(&program, &mut all_lines);

        let mut io = CaptureIo::new();
        let mut obs = CoverageObserver {
            executed: &mut executed,
        };
        let mut interp = Interpreter::with_observer(&mut io, &mut obs);
        match interp.eval_program(&program) {
            Ok(()) => tests_passed += 1,
            Err(diag) => {
                eprintln!("{}", diag.format_with(personality, Some(&source)));
                tests_failed += 1;
            }
        }
    }

    let mut file_covs = Vec::new();
    let mut total_covered = 0usize;
    let mut total_lines = 0usize;

    let mut out = String::from("DOG Coverage Report\n-------------------\n");
    for (file, lines) in &all_lines {
        let tot = lines.len();
        let cov = executed
            .get(file)
            .map(|exec_set| lines.intersection(exec_set).count())
            .unwrap_or(0);
        let pct = if tot == 0 {
            100.0
        } else {
            (cov as f64) * 100.0 / (tot as f64)
        };
        total_covered += cov;
        total_lines += tot;
        out.push_str(&format!("  {file:<32} {cov:>4}/{tot:<4} lines ({pct:>5.1}%)\n"));
        file_covs.push(FileCoverage {
            file: file.clone(),
            covered_lines: cov,
            total_lines: tot,
            percent: pct,
        });
    }

    let total_pct = if total_lines == 0 {
        100.0
    } else {
        (total_covered as f64) * 100.0 / (total_lines as f64)
    };
    out.push_str("-------------------\n");
    out.push_str(&format!(
        "Total: {total_covered}/{total_lines} lines ({total_pct:.1}%) | Tests: {tests_passed} passed, {tests_failed} failed"
    ));

    Ok(CoverageReport {
        tests_passed,
        tests_failed,
        total_covered,
        total_lines,
        percent: total_pct,
        files: file_covs,
        report_text: out,
    })
}

fn collect_program_lines(program: &Program, lines: &mut BTreeMap<String, BTreeSet<usize>>) {
    for stmt in &program.statements {
        collect_stmt_lines(stmt, lines);
    }
}

fn collect_stmt_lines(stmt: &Stmt, lines: &mut BTreeMap<String, BTreeSet<usize>>) {
    match stmt {
        Stmt::Use { .. } | Stmt::StructDef { .. } | Stmt::EnumDef { .. } => {}
        Stmt::FnDef { body, .. } => {
            for s in body {
                collect_stmt_lines(s, lines);
            }
        }
        Stmt::If {
            then_branch,
            otherwise_branch,
            ..
        } => {
            let span = stmt_span(stmt);
            lines
                .entry(span.file.clone())
                .or_default()
                .insert(span.line);
            for s in then_branch {
                collect_stmt_lines(s, lines);
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_stmt_lines(s, lines);
                }
            }
        }
        Stmt::Match {
            cases,
            otherwise_branch,
            ..
        } => {
            let span = stmt_span(stmt);
            lines
                .entry(span.file.clone())
                .or_default()
                .insert(span.line);
            for c in cases {
                for s in &c.body {
                    collect_stmt_lines(s, lines);
                }
            }
            if let Some(el) = otherwise_branch {
                for s in el {
                    collect_stmt_lines(s, lines);
                }
            }
        }
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => {
            let span = stmt_span(stmt);
            lines
                .entry(span.file.clone())
                .or_default()
                .insert(span.line);
            for s in try_body {
                collect_stmt_lines(s, lines);
            }
            for s in catch_body {
                collect_stmt_lines(s, lines);
            }
        }
        Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::For { body, .. } => {
            let span = stmt_span(stmt);
            lines
                .entry(span.file.clone())
                .or_default()
                .insert(span.line);
            for s in body {
                collect_stmt_lines(s, lines);
            }
        }
        _ => {
            let span = stmt_span(stmt);
            lines
                .entry(span.file.clone())
                .or_default()
                .insert(span.line);
        }
    }
}

pub fn collect_test_files(target: Option<&str>) -> Result<Vec<PathBuf>, String> {
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
