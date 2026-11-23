//! CI contract: the tier-1 gate in `.github/workflows/ci.yml` mirrors `just check`.
//!
//! The workflow is data, not Rust behaviour, so this is a string-assert contract
//! test in the same style as `crates/stellwerk-cli/tests/ci_gate.rs`. A missing
//! job, a forgiving step, a shallow checkout or a floating toolchain makes the
//! gate untrustworthy even when every individual command is correct, so each
//! property is asserted explicitly.
//!
//! The expected-RED spec suite is deliberately NOT part of this gate: it lives
//! in a separate workflow (task 19) and must stay out of branch protection.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Locate the workspace root from the compile-time crate directory.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        assert!(
            dir.pop(),
            "no .git found above {}",
            env!("CARGO_MANIFEST_DIR")
        );
    }
}

fn ci_yaml() -> String {
    fs::read_to_string(workspace_root().join(".github/workflows/ci.yml"))
        .expect("read .github/workflows/ci.yml")
}

fn spec_yaml() -> String {
    fs::read_to_string(workspace_root().join(".github/workflows/spec.yml"))
        .expect("read .github/workflows/spec.yml")
}

/// Collect every `-p stellwerk-<crate>` target mentioned in a workflow.
fn test_crates(yaml: &str) -> BTreeSet<String> {
    const NEEDLE: &str = "-p stellwerk-";
    let mut crates = BTreeSet::new();
    let mut rest = yaml;
    while let Some(pos) = rest.find(NEEDLE) {
        let tail = &rest[pos + NEEDLE.len()..];
        let name: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if !name.is_empty() {
            crates.insert(format!("stellwerk-{name}"));
        }
        rest = &tail[name.len()..];
    }
    crates
}

#[test]
fn ci_has_all_tier_one_jobs() {
    let yaml = ci_yaml();
    for job in [
        "fmt",
        "clippy",
        "registry",
        "core",
        "store",
        "risk-tables",
        "scripts",
        "repo-hygiene",
        "history",
        "plan-lint",
    ] {
        assert!(
            yaml.contains(&format!("  {job}:")),
            "missing required tier-1 job `{job}`:\n{yaml}"
        );
    }
}

#[test]
fn ci_runs_the_local_gate_recipes() {
    let yaml = ci_yaml();
    for cmd in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets -- -D warnings",
        "cargo test -p stellwerk-registry --locked",
        "cargo test -p stellwerk-core --locked",
        "cargo test -p stellwerk-store --locked",
        "cargo test -p xtask --test ledger_encoding",
        "just scripts",
        "cargo test -p stellwerk-registry --test repo_hygiene",
        "bash scripts/scan-history.sh",
        "./scripts/plan-lint.sh",
    ] {
        assert!(yaml.contains(cmd), "missing gate command `{cmd}`:\n{yaml}");
    }
}

#[test]
fn ci_is_hard_with_no_forgiving_steps() {
    let yaml = ci_yaml();
    assert!(
        !yaml.contains("|| true"),
        "`|| true` turns a failed check into a green one:\n{yaml}"
    );
    assert!(
        !yaml.contains("continue-on-error"),
        "`continue-on-error` turns a failed check into a green one:\n{yaml}"
    );
}

#[test]
fn ci_uses_full_history_and_pinned_toolchain() {
    let yaml = ci_yaml();
    assert!(
        yaml.contains("fetch-depth: 0"),
        "shallow checkout makes scan-history.sh exit 2:\n{yaml}"
    );
    assert!(
        yaml.contains("dtolnay/rust-toolchain@1.99.0"),
        "toolchain must be pinned to 1.99.0:\n{yaml}"
    );
    assert!(
        !yaml.contains("@stable"),
        "toolchain must not float on @stable:\n{yaml}"
    );
}

#[test]
fn ci_does_not_duplicate_the_expected_red_spec_suite() {
    let yaml = ci_yaml();
    for token in ["stellwerk-task", "stellwerk-gateway", "expected RED"] {
        assert!(
            !yaml.contains(token),
            "`{token}` belongs to the separate RED workflow (task 19), not the hard gate:\n{yaml}"
        );
    }
}

#[test]
fn adr_0027_records_the_gate_boundary() {
    let adr = fs::read_to_string(workspace_root().join("docs/adr/0027-ci-mirrors-local-gate.md"))
        .expect("read ADR-0027");
    for token in ["tier-1", "branch protection", "RED"] {
        assert!(
            adr.contains(token),
            "ADR-0027 must state the `{token}` boundary:\n{adr}"
        );
    }
}

#[test]
fn spec_workflow_exists_and_is_named_expected_red() {
    let yaml = spec_yaml();
    assert!(!yaml.trim().is_empty(), "spec.yml must not be empty");
    assert!(
        yaml.contains("spec (expected RED)"),
        "the RED suite must be visibly named in the workflow:\n{yaml}"
    );
}

#[test]
fn spec_workflow_is_hard_with_no_forgiving_steps() {
    let yaml = spec_yaml();
    assert!(
        !yaml.contains("|| true"),
        "`|| true` would hide a RED spec failure:\n{yaml}"
    );
    assert!(
        !yaml.contains("continue-on-error"),
        "`continue-on-error` would hide a RED spec failure:\n{yaml}"
    );
}

#[test]
fn spec_workflow_runs_exactly_task_and_gateway() {
    let yaml = spec_yaml();
    for cmd in [
        "cargo test -p stellwerk-task --locked --no-fail-fast",
        "cargo test -p stellwerk-gateway --locked --no-fail-fast",
    ] {
        assert!(yaml.contains(cmd), "missing spec command `{cmd}`:\n{yaml}");
    }
    let expected: BTreeSet<String> = ["stellwerk-task", "stellwerk-gateway"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        test_crates(&yaml),
        expected,
        "spec.yml must run exactly task and gateway, no other product crates:\n{yaml}"
    );
}

#[test]
fn spec_workflow_has_full_history_and_red_prerequisites() {
    let yaml = spec_yaml();
    for token in [
        "push",
        "pull_request",
        "fetch-depth: 0",
        "apt-get install -y bubblewrap python3",
        "--no-fail-fast",
    ] {
        assert!(
            yaml.contains(token),
            "missing spec prerequisite `{token}`:\n{yaml}"
        );
    }
}

#[test]
fn spec_workflow_is_not_a_required_check() {
    let yaml = spec_yaml();
    assert!(
        yaml.contains("branch protection"),
        "spec.yml must document that it is not a required check:\n{yaml}"
    );
}
