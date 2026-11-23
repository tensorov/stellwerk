//! Repository hygiene guard for the public tree.
//!
//! Reads the tracked set exclusively through `git ls-files -z` — never a filesystem
//! walk — so untracked internal artifacts (ignored on disk) cannot create false
//! failures, and a file forced back into the index is caught immediately.
//!
//! Forbidden path tokens are assembled at runtime: this file is itself a tracked
//! `crates/**/*.rs` file, so a literal token would make the guard flag its own source.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate the workspace root from the compile-time crate directory.
fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        assert!(dir.pop(), "no .git found above {}", env!("CARGO_MANIFEST_DIR"));
    }
}

/// The authoritative tracked set, NUL-delimited so spaces in paths are safe.
fn tracked(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .expect("git ls-files -z must run");
    assert!(
        out.status.success(),
        "git ls-files -z failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("tracked paths are UTF-8")
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Working-tree content of a tracked path (lossy for binary blobs).
fn content(root: &Path, rel: &str) -> String {
    let bytes = std::fs::read(root.join(rel))
        .unwrap_or_else(|e| panic!("cannot read tracked file {rel}: {e}"));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Build a hidden directory token (`.` + name) without writing it literally.
fn dot(name: &str) -> String {
    format!(".{name}")
}

/// Plan trees are the only places allowed to carry local paths and redacted examples.
fn is_plan(path: &str) -> bool {
    path.starts_with(".omo/plans/") || path.starts_with(".sisyphus/plans/")
}

/// `ssh -p <digits> <host>` where the host carries a real (alphabetic) TLD.
fn has_ssh_endpoint(line: &str, prefix: &str) -> bool {
    let Some(pos) = line.find(prefix) else {
        return false;
    };
    let rest = &line[pos + prefix.len()..];
    let port_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    if port_len == 0 {
        return false;
    }
    let Some(host) = rest[port_len..].strip_prefix(' ') else {
        return false;
    };
    let host = host.split_whitespace().next().unwrap_or("");
    host.split('.')
        .skip(1)
        .any(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// Private `192.168.<n>.<n>` address, assembled so the source has no literal form.
fn has_private_ip(line: &str, net: &str) -> bool {
    let Some(pos) = line.find(net) else {
        return false;
    };
    let rest = &line[pos + net.len()..];
    let third = rest.bytes().take_while(u8::is_ascii_digit).count();
    if third == 0 {
        return false;
    }
    let Some(rest) = rest[third..].strip_prefix('.') else {
        return false;
    };
    rest.bytes().take_while(u8::is_ascii_digit).count() > 0
}

#[test]
fn no_internal_artifact_classes_are_tracked() {
    let root = repo_root();
    let prefixes: Vec<String> = ["night", "stellwerk", "ablation-pi", "mutation-evidence", "PLACEHOLDER"]
        .iter()
        .map(|name| format!("{}/", dot(name)))
        .chain([
            "docs/diagnostics/".to_owned(),
            format!("{}/archive/", dot("omo")),
        ])
        .collect();
    let needles = ["__pycache__/", "mutants.out"];

    let offenders: Vec<String> = tracked(&root)
        .into_iter()
        .filter(|p| {
            prefixes.iter().any(|x| p.starts_with(x)) || needles.iter().any(|x| p.contains(x))
        })
        .collect();
    assert!(offenders.is_empty(), "tracked internal artifacts: {offenders:#?}");
}

#[test]
fn no_local_home_paths_outside_plans() {
    let root = repo_root();
    let needle = format!("/home/{}", "izyabretatel");
    let offenders: Vec<String> = tracked(&root)
        .into_iter()
        .filter(|p| !is_plan(p))
        .filter(|p| content(&root, p).contains(&needle))
        .collect();
    assert!(offenders.is_empty(), "local home path outside plans: {offenders:#?}");
}

#[test]
fn required_public_files_are_tracked() {
    let root = repo_root();
    let files = tracked(&root);
    for required in ["LICENSE-MIT", "LICENSE-APACHE", "SECURITY.md", ".omo/omo.jsonc"] {
        assert!(
            files.iter().any(|p| p == required),
            "required public file is not tracked: {required}"
        );
    }
}

#[test]
fn no_private_omo_runtime_is_tracked() {
    let root = repo_root();
    let prefixes: Vec<String> = ["evidence", "run-continuation", "drafts"]
        .iter()
        .map(|dir| format!("{}/{dir}/", dot("omo")))
        .collect();
    let offenders: Vec<String> = tracked(&root)
        .into_iter()
        .filter(|p| prefixes.iter().any(|x| p.starts_with(x)))
        .collect();
    assert!(offenders.is_empty(), "tracked private OMO runtime: {offenders:#?}");
}

#[test]
fn no_removed_path_references_in_public_sources() {
    let root = repo_root();
    let needles = [
        format!("{}/", dot("night")),
        format!("{}/FINDINGS", dot("stellwerk")),
        format!("{}/DUPLICATES", dot("stellwerk")),
    ];
    let offenders: Vec<String> = tracked(&root)
        .into_iter()
        .filter(|p| {
            (p.starts_with("crates/") && p.ends_with(".rs"))
                || (p.starts_with("docs/") && !p.starts_with("docs/adr/"))
        })
        .filter(|p| {
            let text = content(&root, p);
            needles.iter().any(|n| text.contains(n))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "removed path references in public sources: {offenders:#?}"
    );
}

#[test]
fn no_live_endpoints_in_plans() {
    let root = repo_root();
    let ssh_prefix = format!("{} -p ", "ssh");
    let private_net = format!("{}.{}.", "192", "168");
    let mut offenders: Vec<String> = Vec::new();
    for path in tracked(&root).into_iter().filter(|p| is_plan(p)) {
        let text = content(&root, &path);
        if text
            .lines()
            .any(|line| has_ssh_endpoint(line, &ssh_prefix) || has_private_ip(line, &private_net))
        {
            offenders.push(path);
        }
    }
    assert!(offenders.is_empty(), "live endpoints in plans: {offenders:#?}");
}
