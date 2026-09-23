use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

// ── error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("not implemented")]
    NotImplemented,
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// ── calibration ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibration {
    #[serde(rename = "tokens_per_byte", alias = "bytes_per_token", default)]
    pub tokens_per_byte: f64,
    pub measured_at: String,
}

// ── module kind ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    Mcp,
    Skill,
}

// ── source ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Git {
        git: String,
        rev: String,
    },
    Npm {
        npm: String,
        version: Option<String>,
    },
    Crate {
        #[serde(rename = "cargo")]
        krate: String,
        version: Option<String>,
    },
}

// ── update policy ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePolicy {
    pub channel: String,
    pub policy: String,
}

// ── module ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub kind: ModuleKind,
    #[serde(default = "default_bool_true")]
    pub autoload: bool,
    pub risk: Option<String>,
    pub source: Source,
    pub command: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub context_cost_tokens: u64,
    pub update: Option<UpdatePolicy>,
}

fn default_bool_true() -> bool {
    true
}

// ── registry / modules file ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
    pub budgets: Option<u64>,
    pub calibration: Calibration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModulesFile {
    pub registry: Registry,
    #[serde(default)]
    pub module: BTreeMap<String, Module>,
}

impl ModulesFile {
    /// Parse TOML.
    pub fn from_toml_str(s: &str) -> Result<Self, ModelError> {
        Ok(toml::from_str(s)?)
    }

    /// Read from filesystem.
    pub fn load(path: &Path) -> Result<Self, ModelError> {
        let s = std::fs::read_to_string(path)?;
        Self::from_toml_str(&s)
    }
}

// ── formula ────────────────────────────────────────────────────────────────

/// tokens = round(bytes * tokens_per_byte)
pub fn tokens_for(bytes: u64, tokens_per_byte: f64) -> u64 {
    (bytes as f64 * tokens_per_byte).round() as u64
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
[registry.calibration]
bytes_per_token = 0.5
measured_at = "2025-01-01T00:00:00Z"

[module.polyforge]
kind = "mcp"
source = { git = "https://github.com/example/polyforge", rev = "abc123" }
command = "/run/polyforge"
tools = ["read","write"]
update = { channel = "tag", policy = "pr" }
"#;

    #[test]
    fn from_toml_str_parses_fixture() {
        let mf = ModulesFile::from_toml_str(FIXTURE).expect("fixture must parse");
        assert_eq!(mf.registry.calibration.tokens_per_byte, 0.5);
        assert_eq!(mf.module.len(), 1);
        let poly = &mf.module["polyforge"];
        assert_eq!(poly.kind, ModuleKind::Mcp);
        match &poly.source {
            Source::Git { git, rev } => {
                assert_eq!(git, "https://github.com/example/polyforge");
                assert_eq!(rev, "abc123");
            }
            v => panic!("expected Git variant, got {:?}", v),
        }
        assert!(mf.registry.budgets.is_none());
    }

    #[test]
    fn from_toml_str_rejects_unknown_module_keys() {
        let bad = r#"
[registry.calibration]
bytes_per_token = 0.5
measured_at = "2025-01-01"
[module.x]
kind = "mcp"
source = { git = "https://example.com", rev = "v1" }
bogus_field = "x"
"#;
        let res = ModulesFile::from_toml_str(bad);
        assert!(res.is_err(), "must reject unknown keys");
    }

    #[test]
    fn from_toml_str_rejects_unknown_source_keys() {
        let bad = r#"
[registry.calibration]
bytes_per_token = 0.5
measured_at = "2025-01-01"
[module.x]
kind = "mcp"
source = { foo = "x" }
"#;
        let res = ModulesFile::from_toml_str(bad);
        assert!(res.is_err(), "must reject unknown source keys");
    }

    #[test]
    fn load_reads_file() {
        let dir = std::env::temp_dir().join("stellwerk_test_load");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("modules.toml");
        std::fs::write(&path, FIXTURE).unwrap();
        let mf = ModulesFile::load(&path).expect("load must succeed");
        assert_eq!(mf.module.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tokens_for_formula() {
        assert_eq!(tokens_for(1000, 0.813), 813);
        assert_eq!(tokens_for(0, 0.813), 0);
        assert_eq!(tokens_for(1, 0.5), 1); // 0.5 → 1 (half away from zero)
    }

    fn roundtrip(source: Source) {
        let json = serde_json::to_string(&source).unwrap();
        let restored: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(source, restored);
    }

    #[test]
    fn source_roundtrip_git() {
        roundtrip(Source::Git {
            git: "https://github.com/x/y".into(),
            rev: "v1".into(),
        });
    }

    #[test]
    fn source_roundtrip_npm() {
        roundtrip(Source::Npm {
            npm: "@scope/pkg".into(),
            version: Some("1.0.0".into()),
        });
    }

    #[test]
    fn source_roundtrip_crate() {
        roundtrip(Source::Crate {
            krate: "mylib".into(),
            version: None,
        });
    }
}
