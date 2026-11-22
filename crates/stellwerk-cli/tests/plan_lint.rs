//! L01: the canonical plan must be green under `plan-lint` without `--gating`.
//!
//! A green lint over a subset of packets cannot be told apart from a lint that
//! checked nothing, so the guard runs the real script over the whole canon.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn unique_temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "stellwerk-plan-lint-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp root");
    dir
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
    assert!(
        combined.contains("ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном"),
        "plan-lint did not confirm mirror parity\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        !combined.contains("расхождений"),
        "plan-lint reported mirror drift\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}

#[test]
fn plan_lint_gates_mirror_drift_via_root() {
    let repo_root = workspace_root();
    let tmp = unique_temp_root("mirror-drift");
    let canon_dir = tmp.join(".sisyphus/plans/M3");
    let mirror_dir = tmp.join(".omo/plans/M3");
    fs::create_dir_all(&canon_dir).expect("create canon dir");
    fs::create_dir_all(&mirror_dir).expect("create mirror dir");

    let canon_packet = "\
# packet-04
plan_id: M3
goal: Canon packet for mirror drift gate test.
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --lib
acceptance: cargo test -p stellwerk-core --lib passes.
forbidden: [cargo add]
depends_on: [03a, 03b]
parallelism: sequential-only
data_class: public
lane: m3/test
";
    let mirror_packet = canon_packet.replace("depends_on: [03a, 03b]", "depends_on: [03]");

    fs::write(canon_dir.join("packet-04.md"), canon_packet).expect("write canon packet");
    fs::write(mirror_dir.join("packet-04.md"), mirror_packet).expect("write mirror packet");

    let script = repo_root.join("scripts/plan-lint.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--root")
        .arg(&tmp)
        .output()
        .expect("failed to run plan-lint.py with --root");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 from C8 mirror drift gate\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        combined.contains("ЗЕРКАЛО ПАКЕТОВ: расхождений"),
        "expected C8 mirror drift marker in output\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}
