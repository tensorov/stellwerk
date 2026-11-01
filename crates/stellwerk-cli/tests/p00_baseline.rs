//! P00: keep the measured baseline document from drifting away from what was
//! actually captured in `/tmp/opencode/p00/`.
//!
//! The guard is deliberately line-based: a command and its exit code must sit on
//! one line, so dropping or editing either half fails the test.

use std::fs;
use std::path::{Path, PathBuf};

/// `CARGO_MANIFEST_DIR` is `<root>/crates/stellwerk-cli`; the repo root is two
/// levels up.
fn workspace_root() -> PathBuf {
    match Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
    {
        Some(root) => root.to_path_buf(),
        None => panic!("CARGO_MANIFEST_DIR is not two levels below the workspace root"),
    }
}

/// First line that contains every non-empty needle.
fn line_with_all<'a>(document: &'a str, needles: &[&str]) -> Option<&'a str> {
    document.lines().find(|line| {
        needles
            .iter()
            .all(|needle| !needle.is_empty() && line.contains(needle))
    })
}

#[test]
fn baseline_lists_every_measured_command() {
    let path = workspace_root().join("docs/p00-baseline.md");
    let document = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    };

    let required_lines: [(&str, &[&str]); 6] = [
        ("fmt", &["cargo fmt --all -- --check", "exit 1"]),
        (
            "clippy",
            &[
                "cargo clippy --workspace --all-targets -- -D warnings",
                "exit 101",
            ],
        ),
        ("test", &["cargo test --workspace --no-fail-fast", "exit 0"]),
        ("plan-lint", &["just plan-lint", "exit 1", "77", "RED"]),
        (
            "replay",
            &["python3 scripts/packet_replay.py inventory", "exit 0"],
        ),
        ("doctor", &["just doctor", "exit 0"]),
    ];

    for (id, needles) in required_lines {
        assert!(
            line_with_all(&document, needles).is_some(),
            "{}: no single line for `{id}` contains {needles:?}",
            path.display()
        );
    }

    for crate_name in [
        "stellwerk-core",
        "stellwerk-gateway",
        "stellwerk-registry",
        "stellwerk-store",
        "stellwerk-task",
        "stellwerk-cli",
    ] {
        assert!(
            document.contains(crate_name),
            "{}: missing crate name `{crate_name}`",
            path.display()
        );
    }

    assert!(
        document.contains("0.87.0"),
        "{}: missing the installed Pi version 0.87.0",
        path.display()
    );
}
