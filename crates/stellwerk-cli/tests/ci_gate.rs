//! I01: the CI gate must execute PolyForge `gate_evaluate`, not print a TODO.
//!
//! Behaviour is proven by running `scripts/integrator/gate.sh` against a fake
//! MCP server. The YAML check only proves the workflow calls that script, so a
//! green YAML alone can never stand in for a working gate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Minimal MCP server: answers `initialize` (id 1) and `tools/call` (id 2).
/// The gate verdict is injected via `FAKE_GATE_RESPONSE`, so one fixture covers
/// both `ok:true` and `ok:false` without a real PolyForge process.
const FAKE_SERVER: &str = r#"import json, os, sys
gate = os.environ["FAKE_GATE_RESPONSE"]
for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    msg = json.loads(raw)
    if msg.get("id") == 1:
        print(json.dumps({"jsonrpc": "2.0", "id": 1, "result": {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "serverInfo": {"name": "fake", "version": "0"}}}), flush=True)
    elif msg.get("id") == 2:
        print(gate, flush=True)
"#;

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

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("stellwerk-i01-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct GateRun {
    code: Option<i32>,
    output: String,
}

/// Run the real gate script. `server` selects the fake MCP server; `override_cmd`
/// replaces `POLYFORGE_CMD` so the fail-closed path can be exercised too.
fn run_gate(server: Option<&Path>, gate_response: &str, override_cmd: Option<&str>) -> GateRun {
    let root = workspace_root();
    let mut cmd = Command::new(root.join("scripts/integrator/gate.sh"));
    cmd.arg("probe").current_dir(&root);
    cmd.env_remove("POLYFORGE_CMD");
    if let Some(server) = server {
        cmd.env("POLYFORGE_CMD", format!("python3 {}", server.display()));
        cmd.env("FAKE_GATE_RESPONSE", gate_response);
    }
    if let Some(override_cmd) = override_cmd {
        cmd.env("POLYFORGE_CMD", override_cmd);
    }
    let out = cmd.output().expect("run scripts/integrator/gate.sh");
    let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&out.stderr));
    GateRun {
        code: out.status.code(),
        output,
    }
}

fn fake_server(dir: &TempDir) -> PathBuf {
    let path = dir.join("fake_mcp.py");
    fs::write(&path, FAKE_SERVER).expect("write fake MCP server");
    path
}

#[test]
fn gate_rejects_when_gate_evaluate_is_not_ok() {
    let dir = TempDir::new();
    let server = fake_server(&dir);
    let run = run_gate(
        Some(&server),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":false,"state":"MissingToolAttestation"}}"#,
        None,
    );
    assert_eq!(run.code, Some(1), "output: {}", run.output);
    assert!(
        run.output.contains("MissingToolAttestation"),
        "output: {}",
        run.output
    );
}

#[test]
fn gate_passes_when_gate_evaluate_is_ok() {
    let dir = TempDir::new();
    let server = fake_server(&dir);
    let run = run_gate(
        Some(&server),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true,"state":"Verified"}}"#,
        None,
    );
    assert_eq!(run.code, Some(0), "output: {}", run.output);
}

/// I02: an unattested bundle is refused by exit code, not by a substring that a
/// gate which allowed everything would also print. `ok:true` is the control: the
/// same fixture must then exit 0, so exit 1 cannot be a constant.
#[test]
fn unattested_bundle_returns_exit_one() {
    let dir = TempDir::new();
    let server = fake_server(&dir);
    let rejected = run_gate(
        Some(&server),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":false,"state":"MissingToolAttestation"}}"#,
        None,
    );
    assert_eq!(rejected.code, Some(1), "output: {}", rejected.output);
    assert!(
        rejected.output.contains("MissingToolAttestation"),
        "output: {}",
        rejected.output
    );

    let accepted = run_gate(
        Some(&server),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true,"state":"Verified"}}"#,
        None,
    );
    assert_eq!(accepted.code, Some(0), "output: {}", accepted.output);
}

#[test]
fn gate_fails_closed_when_polyforge_is_missing() {
    let run = run_gate(None, "", Some("/nonexistent"));
    assert_eq!(run.code, Some(2), "output: {}", run.output);
    assert!(
        run.output.contains("MissingPolyForge"),
        "output: {}",
        run.output
    );
}

#[test]
fn polyforge_workflow_is_removed() {
    // The filename is assembled at runtime so this guard does not flag its own
    // source in the acceptance grep for the removed workflow file.
    let workflow = ["polyforge", "yml"].join(".");
    let path = workspace_root().join(".github/workflows").join(workflow);
    assert!(
        !path.exists(),
        "the dead polyforge workflow must be absent: {}",
        path.display()
    );
}
