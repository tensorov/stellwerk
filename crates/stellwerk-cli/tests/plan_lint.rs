//! L01: the canonical plan must be green under `plan-lint` without `--gating`.
//!
//! A green lint over a subset of packets cannot be told apart from a lint that
//! checked nothing, so the guard runs the real script over the whole canon.

use std::path::{Path, PathBuf};
use std::process::Command;

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

#[test]
fn plan_lint_is_green() {
    let root = workspace_root();
    let output = Command::new("python3")
        .arg("scripts/plan-lint.py")
        .current_dir(&root)
        .output()
        .expect("failed to run `python3 scripts/plan-lint.py`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        output.status.success(),
        "plan-lint exited with {:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status
    );
    assert!(
        !combined.contains("НАРУШЕНИЙ"),
        "plan-lint still reports violations\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        combined.contains("порядок объявлен везде"),
        "plan-lint did not declare cross-milestone order\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}
