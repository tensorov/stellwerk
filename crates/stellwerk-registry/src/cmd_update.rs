//! Module update command — packet-11, minimal scope.
//!
//! Pipeline (M0): `fetch → build → smoke → measure → gh pr create`. Never merges.
//! Only `git` sources are supported in this packet; npm/cargo sources are rejected
//! explicitly rather than silently skipped.
//!
//! Every external binary is injectable so the pipeline is testable without a network:
//!   `STELLWERK_GIT_BIN`, `STELLWERK_CARGO_BIN`, `STELLWERK_GH_BIN`.
//!
//! On any failure the intended `modules.lock` change is written as a patch to
//! `.stellwerk/pending/<name>.patch` and the command exits non-zero.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cli::NameArgs;
use crate::cmd_measure;
use crate::model::{ModulesFile, Source};

/// `audit` lands in packet-09. Until then the smoke step is the MCP handshake that
/// `measure` performs (initialize → initialized → tools/list), and audit is reported
/// as pending instead of being silently claimed.
const AUDIT_IMPLEMENTED: bool = false;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("module required: stellwerk module update <name>")]
    NameRequired,
    #[error("no such module: {0}")]
    NoModule(String),
    #[error("update supports git sources only, {0} is not a git source")]
    NotGitSource(String),
    #[error("module {0} has no command — cannot smoke it")]
    NoCommand(String),
    #[error("command failed: {bin} {args}: {stderr}")]
    Command {
        bin: String,
        args: String,
        stderr: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Model(#[from] crate::model::ModelError),
    #[error(transparent)]
    Measure(#[from] cmd_measure::MeasureError),
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

/// External binaries used by the pipeline.
///
/// Passed explicitly instead of read from the environment deep inside the steps:
/// environment variables are process-global, so tests that mutate them race each other
/// under the default parallel test runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bins {
    pub git: String,
    pub cargo: String,
    pub gh: String,
}

impl Bins {
    /// Real invocation: read the overrides, fall back to PATH.
    pub fn from_env() -> Self {
        Self {
            git: env_or("STELLWERK_GIT_BIN", "git"),
            cargo: env_or("STELLWERK_CARGO_BIN", "cargo"),
            gh: env_or("STELLWERK_GH_BIN", "gh"),
        }
    }
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<String, UpdateError> {
    let out = Command::new(bin).args(args).stdin(Stdio::null()).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let trimmed: String = stderr.trim().chars().take(400).collect();
        return Err(UpdateError::Command {
            bin: bin.to_owned(),
            args: args.join(" "),
            stderr: trimmed,
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ── pending patch ──────────────────────────────────────────────────────────

/// Write the intended lock change as a full-file unified diff under
/// `.stellwerk/pending/`, so a human can apply it after reviewing the failure.
pub fn write_pending(
    root: &Path,
    name: &str,
    before: &str,
    after: &str,
    reason: &str,
) -> Result<PathBuf, UpdateError> {
    let dir = root.join(".stellwerk").join("pending");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{name}.patch"));

    let before_body: String = before.lines().map(|l| format!("-{l}\n")).collect();
    let after_body: String = after.lines().map(|l| format!("+{l}\n")).collect();
    let patch = format!(
        "# stellwerk pending update: {name}\n\
         # reason: {reason}\n\
         # apply with: git apply .stellwerk/pending/{name}.patch\n\
         --- a/modules.lock\n\
         +++ b/modules.lock\n\
         @@ -1,{} +1,{} @@\n\
         {}{}",
        before.lines().count().max(1),
        after.lines().count().max(1),
        if before.is_empty() {
            String::new()
        } else {
            before_body
        },
        after_body,
    );
    std::fs::write(&path, patch)?;
    Ok(path)
}

// ── pipeline steps ─────────────────────────────────────────────────────────

fn fetch_source(git: &str, url: &str, rev: &str, dest: &Path) -> Result<(), UpdateError> {
    std::fs::create_dir_all(dest)?;
    // full clone (no --depth): a shallow clone may not contain the pinned rev
    run_cmd(git, &["clone", url, dest.to_str().unwrap_or_default()])?;
    run_cmd(
        git,
        &["-C", dest.to_str().unwrap_or_default(), "checkout", rev],
    )?;
    Ok(())
}

fn build_if_rust(cargo: &str, dest: &Path) -> Result<bool, UpdateError> {
    let manifest = dest.join("Cargo.toml");
    if !manifest.exists() {
        return Ok(false); // not a Rust module — nothing to build
    }
    run_cmd(
        cargo,
        &[
            "build",
            "--manifest-path",
            manifest.to_str().unwrap_or_default(),
        ],
    )?;
    Ok(true)
}

fn open_pr(
    gh: &str,
    name: &str,
    rev: &str,
    report: &cmd_measure::MeasureReport,
) -> Result<(), UpdateError> {
    let title = format!("chore(module): update {name} to {rev}");
    let body = format!(
        "Automatic update from `stellwerk module update {name}`.\n\n\
         - source revision: `{rev}`\n\
         - schema_bytes: {}\n\
         - context_cost_tokens: {}\n\
         - calibration: from `[registry.calibration]` in modules.toml\n\n\
         Built, then measured over a real MCP handshake. No merge performed by this tool.\n",
        report.schema_bytes, report.context_cost_tokens
    );
    run_cmd(gh, &["pr", "create", "--title", &title, "--body", &body])?;
    Ok(())
}

// ── run ────────────────────────────────────────────────────────────────────

/// Run the `module update` command with binaries taken from the environment.
pub fn run(args: &NameArgs, root: &Path) -> Result<(), UpdateError> {
    run_with(args, root, &Bins::from_env())
}

/// Run the `module update` command with explicit binaries.
pub fn run_with(args: &NameArgs, root: &Path, bins: &Bins) -> Result<(), UpdateError> {
    let name = args
        .name
        .as_deref()
        .ok_or(UpdateError::NameRequired)?
        .to_owned();

    let mf = ModulesFile::load(&root.join("modules.toml"))?;
    let module = mf
        .module
        .get(&name)
        .ok_or_else(|| UpdateError::NoModule(name.clone()))?;

    let (url, rev) = match &module.source {
        Source::Git { git, rev } => (git.clone(), rev.clone()),
        other => return Err(UpdateError::NotGitSource(format!("{} ({other:?})", name))),
    };
    let command = module
        .command
        .clone()
        .ok_or_else(|| UpdateError::NoCommand(name.clone()))?;

    let lock_path = root.join("modules.lock");
    let before = std::fs::read_to_string(&lock_path).unwrap_or_else(|_| "version = 1\n".to_owned());

    let dest = std::env::temp_dir().join(format!("stellwerk-update-{name}-{}", std::process::id()));

    let outcome = pipeline(PipelineArgs {
        root,
        name: &name,
        url: &url,
        rev: &rev,
        command: &command,
        dest: &dest,
        bins,
    });

    match outcome {
        Ok(report) => {
            // PR gate: if gh fails we still owe the human the measured change.
            if let Err(e) = open_pr(&bins.gh, &name, &rev, &report) {
                let after = std::fs::read_to_string(&lock_path).unwrap_or_default();
                let path = write_pending(root, &name, &before, &after, &e.to_string())?;
                let _ = std::fs::remove_dir_all(&dest);
                return Err(UpdateError::Command {
                    bin: "pending".to_owned(),
                    args: path.display().to_string(),
                    stderr: format!("{e}"),
                });
            }
            let _ = std::fs::remove_dir_all(&dest);
            println!(
                "update {}: rev={} schema_bytes={} context_cost_tokens={} (PR opened)",
                name, rev, report.schema_bytes, report.context_cost_tokens
            );
            Ok(())
        }
        Err(e) => {
            // Measure may have already written the lock; capture it either way.
            let after = match &e {
                UpdateError::Measure(m) => {
                    let _ = m;
                    std::fs::read_to_string(&lock_path).unwrap_or_else(|_| before.clone())
                }
                _ => before.clone(),
            };
            let path = write_pending(root, &name, &before, &after, &e.to_string())?;
            let _ = std::fs::remove_dir_all(&dest);
            Err(UpdateError::Command {
                bin: "pending".to_owned(),
                args: path.display().to_string(),
                stderr: format!("{e}"),
            })
        }
    }
}

struct PipelineArgs<'a> {
    root: &'a Path,
    name: &'a str,
    url: &'a str,
    rev: &'a str,
    command: &'a str,
    dest: &'a Path,
    bins: &'a Bins,
}

fn pipeline(args: PipelineArgs<'_>) -> Result<cmd_measure::MeasureReport, UpdateError> {
    // 1. fetch
    fetch_source(&args.bins.git, args.url, args.rev, args.dest)?;
    // 2. build
    if build_if_rust(&args.bins.cargo, args.dest)? {
        println!("update {}: cargo build ok", args.name);
    } else {
        println!("update {}: no Cargo.toml, build skipped", args.name);
    }
    // 3. smoke — audit is packet-09; the MCP handshake below is the real smoke
    if AUDIT_IMPLEMENTED {
        println!("update {}: audit ok", args.name);
    } else {
        println!(
            "update {}: audit DEFERRED (packet-09), smoke = MCP handshake",
            args.name
        );
    }
    // 4. measure (also writes modules.lock)
    let report = cmd_measure::measure_module(args.root, args.name, args.command)?;
    Ok(report)
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK_BEFORE: &str = "version = 1\n\n[polyforge]\ncontext_cost_tokens = 100\n";

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stellwerk_update_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    fn write_exe(path: &Path, body: &str) {
        std::fs::write(path, body).expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(path).expect("stat").permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(path, perm).expect("chmod");
        }
    }

    /// Fake `git`: `clone <url> <dir>` materialises a tiny Rust project,
    /// `checkout` succeeds.
    fn fake_git(dir: &Path) -> PathBuf {
        let p = dir.join("git.sh");
        write_exe(
            &p,
            "#!/bin/sh\n\
             if [ \"$1\" = \"clone\" ]; then\n\
             \x20 d=\"$3\"; mkdir -p \"$d/src\"\n\
             \x20 printf '[package]\\nname=\"m\"\\nversion=\"0.0.1\"\\nedition=\"2021\"\\n' > \"$d/Cargo.toml\"\n\
             \x20 printf 'pub fn f() {}\\n' > \"$d/src/lib.rs\"\n\
             \x20 exit 0\n\
             fi\n\
             exit 0\n",
        );
        p
    }

    /// Fake MCP server for the measure/smoke step (newline-delimited JSON-RPC).
    fn fake_mcp(dir: &Path) -> PathBuf {
        let p = dir.join("mcp.sh");
        write_exe(
            &p,
            "#!/bin/sh\n\
             while IFS= read -r line; do\n\
             \x20 case \"$line\" in\n\
             \x20   *initialize*) printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{}}}' ;;\n\
             \x20   *tools/list*) printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"t\",\"inputSchema\":{\"type\":\"object\"}}]}}' ;;\n\
             \x20 esac\n\
             done\n",
        );
        p
    }

    /// `source` is the complete TOML assignment, braces included.
    fn modules_toml(command: &str, source: &str) -> String {
        format!(
            "[registry.calibration]\ntokens_per_byte = 0.5\nmeasured_at = \"2026-01-01T00:00:00Z\"\n\n\
             [module.mod]\nkind = \"mcp\"\n{source}\ncommand = \"{command}\"\n"
        )
    }

    fn git_source() -> &'static str {
        "source = { git = \"https://example.com/m\", rev = \"v1\" }"
    }

    #[test]
    fn requires_name() {
        let dir = tmpdir("no_name");
        let res = run(&NameArgs { name: None }, &dir);
        assert!(matches!(res, Err(UpdateError::NameRequired)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_module_is_error() {
        let dir = tmpdir("unknown");
        std::fs::write(
            dir.join("modules.toml"),
            modules_toml("/bin/true", git_source()),
        )
        .expect("write modules.toml");
        let res = run(
            &NameArgs {
                name: Some("nope".to_owned()),
            },
            &dir,
        );
        assert!(matches!(res, Err(UpdateError::NoModule(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_git_source_is_rejected() {
        let dir = tmpdir("npm_source");
        std::fs::write(
            dir.join("modules.toml"),
            modules_toml("/bin/true", "source = { npm = \"pkg\", version = \"1.0\" }"),
        )
        .expect("write modules.toml");
        let res = run(
            &NameArgs {
                name: Some("mod".to_owned()),
            },
            &dir,
        );
        match res {
            Err(UpdateError::NotGitSource(n)) => {
                assert!(n.starts_with("mod"), "payload names the module: {n}");
                assert!(n.contains("Npm"), "payload shows the offending source: {n}");
            }
            other => panic!("expected NotGitSource, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_command_is_error() {
        let dir = tmpdir("no_command");
        let toml = "[registry.calibration]\ntokens_per_byte = 0.5\nmeasured_at = \"2026-01-01T00:00:00Z\"\n\n\
                    [module.mod]\nkind = \"mcp\"\nsource = { git = \"u\", rev = \"v\" }\n";
        std::fs::write(dir.join("modules.toml"), toml).expect("write modules.toml");
        let res = run(
            &NameArgs {
                name: Some("mod".to_owned()),
            },
            &dir,
        );
        assert!(matches!(res, Err(UpdateError::NoCommand(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn success_opens_pr_and_leaves_no_pending() {
        let dir = tmpdir("success");
        let mcp = fake_mcp(&dir);
        let gh_log = dir.join("gh.log");
        let gh = dir.join("gh.sh");
        write_exe(
            &gh,
            &format!(
                "#!/bin/sh\nprintf '%s ' \"$@\" >> '{}'\nexit 0\n",
                gh_log.display()
            ),
        );
        let bins = Bins {
            git: fake_git(&dir).display().to_string(),
            cargo: "/bin/true".to_owned(),
            gh: gh.display().to_string(),
        };
        std::fs::write(
            dir.join("modules.toml"),
            modules_toml(mcp.to_str().expect("utf8"), git_source()),
        )
        .expect("write modules.toml");
        std::fs::write(dir.join("modules.lock"), LOCK_BEFORE).expect("write lock");

        run_with(
            &NameArgs {
                name: Some("mod".to_owned()),
            },
            &dir,
            &bins,
        )
        .expect("update must succeed");

        let logged = std::fs::read_to_string(&gh_log).expect("gh log");
        assert!(logged.contains("pr create"), "must call gh pr create");
        assert!(
            logged.contains("context_cost_tokens"),
            "PR body must carry the measured cost"
        );
        let lock = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert!(lock.contains("[mod]"), "lock records the module");
        assert!(
            !dir.join(".stellwerk/pending").exists(),
            "no pending on success"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gh_failure_writes_pending_patch() {
        let dir = tmpdir("gh_fail");
        let mcp = fake_mcp(&dir);
        let gh = dir.join("gh.sh");
        write_exe(&gh, "#!/bin/sh\necho 'gh: auth required' >&2\nexit 1\n");
        let bins = Bins {
            git: fake_git(&dir).display().to_string(),
            cargo: "/bin/true".to_owned(),
            gh: gh.display().to_string(),
        };
        std::fs::write(
            dir.join("modules.toml"),
            modules_toml(mcp.to_str().expect("utf8"), git_source()),
        )
        .expect("write modules.toml");
        std::fs::write(dir.join("modules.lock"), LOCK_BEFORE).expect("write lock");

        let res = run_with(
            &NameArgs {
                name: Some("mod".to_owned()),
            },
            &dir,
            &bins,
        );
        assert!(res.is_err(), "gh failure must fail the command");
        let patch = dir.join(".stellwerk/pending/mod.patch");
        assert!(patch.exists(), "pending patch must be written");
        let body = std::fs::read_to_string(&patch).expect("patch");
        assert!(body.contains("--- a/modules.lock"), "patch header");
        assert!(body.contains("+[mod]"), "patch carries the new module");
        assert!(
            body.contains("gh: auth required"),
            "patch records the reason"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fetch_failure_writes_pending_patch() {
        let dir = tmpdir("fetch_fail");
        let mcp = fake_mcp(&dir);
        let git = dir.join("git.sh");
        write_exe(
            &git,
            "#!/bin/sh\necho 'fatal: repository not found' >&2\nexit 128\n",
        );
        let bins = Bins {
            git: git.display().to_string(),
            cargo: "/bin/true".to_owned(),
            gh: "/bin/true".to_owned(),
        };
        std::fs::write(
            dir.join("modules.toml"),
            modules_toml(
                mcp.to_str().expect("utf8"),
                "source = { git = \"https://example.com/missing\", rev = \"v1\" }",
            ),
        )
        .expect("write modules.toml");
        std::fs::write(dir.join("modules.lock"), LOCK_BEFORE).expect("write lock");

        let res = run_with(
            &NameArgs {
                name: Some("mod".to_owned()),
            },
            &dir,
            &bins,
        );
        assert!(res.is_err(), "fetch failure must fail the command");
        let patch = dir.join(".stellwerk/pending/mod.patch");
        assert!(
            patch.exists(),
            "pending patch must be written on fetch failure"
        );
        let body = std::fs::read_to_string(&patch).expect("patch");
        assert!(
            body.contains("repository not found"),
            "patch records the git failure"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pending_patch_is_a_valid_full_replacement() {
        let dir = tmpdir("patch_shape");
        let before = "version = 1\n";
        let after = "version = 1\n\n[mod]\ncontext_cost_tokens = 42\n";
        let path = write_pending(&dir, "mod", before, after, "unit").expect("write_pending");
        let body = std::fs::read_to_string(&path).expect("patch");
        assert!(body.contains("@@ -1,1 +1,4 @@"), "hunk header counts lines");
        assert!(body.contains("-version = 1"), "removes old line");
        assert!(body.contains("+context_cost_tokens = 42"), "adds new line");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
