//! `cargo xtask parity` — drift checker for the adaq-data-stock-us ↔ yfinance parity.
//!
//! Diffs the vendored `yfinance` submodule (pinned in `PARITY_PIN`) against its
//! current checked-out commit, cross-references `docs/PARITY.md`, and flags Rust
//! modules whose upstream source changed while still marked done/partial. Also
//! warns when a newer yfinance release tag exists than the pinned version.
//!
//! See `docs/adr/0003-parity-mechanism.md`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("parity") => parity(),
        _ => {
            eprintln!("usage: cargo xtask <command>");
            eprintln!("  parity   check adaq-data-stock-us against the pinned yfinance submodule");
            std::process::exit(2);
        }
    }
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for this xtask crate is <root>/xtask, so the parent is the repo root.
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    Path::new(&manifest)
        .parent()
        .expect("xtask has no parent dir")
        .to_path_buf()
}

fn git(submodule: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(submodule)
        .args(args)
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => {
            eprintln!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&o.stderr)
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("failed to run git: {e}");
            std::process::exit(1);
        }
    }
}

fn parity() {
    let root = repo_root();
    let submodule = root.join("vendor").join("yfinance");
    if !submodule.exists() {
        eprintln!(
            "yfinance submodule not found at {} — run `git submodule update --init` first.",
            submodule.display()
        );
        std::process::exit(1);
    }

    let parity_md = root.join("docs").join("PARITY.md");
    let md = std::fs::read_to_string(&parity_md).unwrap_or_default();

    // Pinned commit and version.
    let pinned_commit = std::fs::read_to_string(root.join("PARITY_PIN"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let pinned_version = extract_field(&md, "Pinned version");

    let target_commit = git(&submodule, &["rev-parse", "HEAD"]);

    println!("=== adaq-data-stock-us ↔ yfinance parity ===");
    if let Some(v) = &pinned_version {
        println!("Pinned yfinance version: {v}");
    }
    if let Some(c) = &pinned_commit {
        println!("Pinned commit:           {c}");
    } else {
        println!("Pinned commit:           (PARITY_PIN missing — diff disabled)");
    }
    println!("Submodule at:             {target_commit}");

    // New-release warning.
    if let Some(latest) = latest_tag(&submodule) {
        match (&pinned_version, parse_ver(&latest)) {
            (Some(pv), Some(lv)) if parse_ver(pv) < Some(lv) => {
                println!("⚠ Newer yfinance release available: {latest} (pinned: {pv})");
            }
            _ => println!("yfinance latest tag:      {latest}"),
        }
    }

    // Drift diff.
    if let Some(pinned) = &pinned_commit {
        if pinned == &target_commit {
            println!("\nSubmodule is at the pinned commit — no upstream drift.");
            return;
        }
        let commits = git(
            &submodule,
            &["rev-list", "--count", &format!("{pinned}..{target_commit}")],
        );
        println!("\nUpstream drift: +{commits} commits since pin.");
        let changed = git(&submodule, &["diff", "--name-only", pinned, &target_commit]);
        let files: Vec<&str> = changed.lines().filter(|l| !l.is_empty()).collect();
        if files.is_empty() {
            println!("(no file changes between pin and submodule HEAD)");
            return;
        }
        println!("Changed upstream files ({}):", files.len());
        for f in &files {
            println!("  - {f}");
        }

        // Cross-reference PARITY.md.
        let rows = parse_parity_rows(&md);
        let mut review: Vec<&ParityRow> = Vec::new();
        let mut todo_affected: Vec<&ParityRow> = Vec::new();
        for f in &files {
            let norm = f.trim_start_matches("yfinance/");
            for r in &rows {
                if r.sources
                    .iter()
                    .any(|s| norm == s.as_str() || norm.ends_with(&format!("/{s}")))
                {
                    if r.status != "todo" {
                        review.push(r);
                    } else {
                        todo_affected.push(r);
                    }
                }
            }
        }
        if !review.is_empty() {
            println!("\n⚠ Modules to REVIEW (upstream changed, status != todo):");
            for r in &review {
                println!(
                    "  - {}  [{}]  (status: {})",
                    r.module,
                    r.sources.join(", "),
                    r.status
                );
            }
        } else {
            println!("\nNo done/partial Rust module is affected by upstream drift. ✔");
        }
        if !todo_affected.is_empty() {
            println!("\nUpstream changed files affecting not-yet-implemented modules:");
            for r in &todo_affected {
                println!("  - {}  [{}]", r.module, r.sources.join(", "));
            }
        }
    }
}

struct ParityRow {
    module: String,
    sources: Vec<String>,
    status: String,
}

fn extract_field(md: &str, key: &str) -> Option<String> {
    md.lines()
        .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
        .map(|v| v.trim().to_string())
}

fn parse_parity_rows(md: &str) -> Vec<ParityRow> {
    let mut rows = Vec::new();
    for line in md.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
        // Expect: ["", module, sources, phase, status, ""]
        if cols.len() < 5 {
            continue;
        }
        let module = cols[1];
        let sources = cols[2];
        let status = cols[4];
        if module.is_empty() || sources.is_empty() || status.is_empty() {
            continue;
        }
        if module.eq_ignore_ascii_case("rust module") {
            continue; // header row
        }
        if status.chars().all(|c| c == '-') {
            continue; // separator row
        }
        rows.push(ParityRow {
            module: module.to_string(),
            sources: sources
                .split(',')
                .map(|s| s.trim().trim_start_matches("yfinance/").to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            status: status.to_string(),
        });
    }
    rows
}

fn latest_tag(submodule: &Path) -> Option<String> {
    let tags = git(submodule, &["tag", "--sort=-v:refname"]);
    tags.lines()
        .filter(|t| parse_ver(t).is_some())
        .next()
        .map(|s| s.to_string())
}

fn parse_ver(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() == 3 {
        let a = parts[0].parse().ok()?;
        let b = parts[1].parse().ok()?;
        let c = parts[2].parse().ok()?;
        Some((a, b, c))
    } else {
        None
    }
}
