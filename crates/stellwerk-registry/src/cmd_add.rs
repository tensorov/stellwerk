//! Module add command — packet-05.

use std::path::Path;

use crate::cli::AddArgs;
use crate::model::ModulesFile;

// ── error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("exactly one source required")]
    OneSource,
    #[error("mcp requires --command")]
    McpNeedsCommand,
    #[error("skill must not have --command")]
    SkillNoCommand,
    #[error("duplicate module: {0}")]
    Duplicate(String),
    #[error("source git requires --rev")]
    GitNeedsRev,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    Model(#[from] crate::model::ModelError),
}

// ── name validation ────────────────────────────────────────────────────────

fn validate_name(name: &str) -> Result<(), AddError> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return Err(AddError::InvalidName(name.to_owned()));
    }
    // first byte: [a-z0-9]
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(AddError::InvalidName(name.to_owned()));
    }
    // rest: [a-z0-9_-]
    for &b in &bytes[1..] {
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') {
            return Err(AddError::InvalidName(name.to_owned()));
        }
    }
    Ok(())
}

// ── source resolution ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct SourceSpec {
    git: Option<String>,
    rev: Option<String>,
    npm: Option<String>,
    npm_version: Option<String>,
    cargo: Option<String>,
    cargo_version: Option<String>,
}

fn resolve_source(args: &AddArgs) -> Result<SourceSpec, AddError> {
    let git_count = if args.git.is_some() { 1 } else { 0 };
    let npm_count = if args.npm.is_some() { 1 } else { 0 };
    let cargo_count = if args.cargo.is_some() { 1 } else { 0 };
    let total = git_count + npm_count + cargo_count;

    if total == 0 {
        return Err(AddError::OneSource);
    }
    if total > 1 {
        return Err(AddError::OneSource);
    }

    Ok(SourceSpec {
        git: args.git.clone(),
        rev: args.rev.clone(),
        npm: args.npm.clone(),
        npm_version: args.npm_version.clone().into(),
        cargo: args.cargo.clone(),
        cargo_version: args.cargo_version.clone().into(),
    })
}

// ── run ────────────────────────────────────────────────────────────────────

/// Run the `module add` command. `root` is the directory containing `modules.toml`.
pub fn run(args: &AddArgs, root: &Path) -> Result<(), AddError> {
    // 1. validate name
    validate_name(&args.name)?;

    // 2. validate source (exactly one)
    let src = resolve_source(args)?;

    // 3. kind-specific command validation
    let is_mcp = matches!(args.kind, crate::cli::KindArg::Mcp);
    if is_mcp && args.command.as_ref().is_none_or(|c| c.is_empty()) {
        return Err(AddError::McpNeedsCommand);
    }
    if !is_mcp && args.command.is_some() {
        return Err(AddError::SkillNoCommand);
    }

    // 4. git requires rev
    if src.git.is_some() && src.rev.is_none() {
        return Err(AddError::GitNeedsRev);
    }

    // 5. duplicate check
    let path = root.join("modules.toml");
    let existing = ModulesFile::load(&path)?;
    if existing.module.contains_key(&args.name) {
        return Err(AddError::Duplicate(args.name.clone()));
    }

    // 6. read existing file as string
    let existing_text = std::fs::read_to_string(&path).unwrap_or_default();

    // 7. build new block
    let block = build_module_block(args, &src);

    // 8. append
    let appended = if existing_text.is_empty() {
        block
    } else {
        format!("\n\n{}", block)
    };

    std::fs::write(&path, format!("{}{}", existing_text, appended))?;

    Ok(())
}

fn build_module_block(args: &AddArgs, src: &SourceSpec) -> String {
    let name = &args.name;
    let kind_str = match args.kind {
        crate::cli::KindArg::Mcp => "mcp",
        crate::cli::KindArg::Skill => "skill",
    };
    let autoload = if args.no_autoload { "false" } else { "true" };
    let risk = args.risk.as_deref().unwrap_or("R3");

    let source_line = match (src.git.as_deref(), src.npm.as_deref(), src.cargo.as_deref()) {
        (Some(g), None, None) => {
            format!(
                r#"source = {{ git = "{}", rev = "{}" }}"#,
                g,
                src.rev.as_deref().unwrap_or("")
            )
        }
        (None, Some(n), None) => {
            if let Some(v) = &src.npm_version {
                format!(r#"source = {{ npm = "{}", version = "{}" }}"#, n, v)
            } else {
                format!(r#"source = {{ npm = "{}" }}"#, n)
            }
        }
        (None, None, Some(c)) => {
            if let Some(v) = &src.cargo_version {
                format!(r#"source = {{ cargo = "{}", version = "{}" }}"#, c, v)
            } else {
                format!(r#"source = {{ cargo = "{}" }}"#, c)
            }
        }
        _ => unreachable!(),
    };

    let mut lines = Vec::new();
    lines.push(format!("[module.{}]", name));
    lines.push(format!("kind     = \"{}\"", kind_str));
    lines.push(format!("autoload = {}", autoload));
    lines.push(format!("risk     = \"{}\"", risk));
    lines.push(source_line);

    if let Some(cmd) = &args.command {
        lines.push(format!("command  = \"{}\"", escape_toml_string(cmd)));
    }

    // tools
    let tools = &args.tools;
    if tools.is_empty() {
        lines.push("tools    = []".to_owned());
    } else {
        let items: Vec<String> = tools
            .iter()
            .map(|t| format!("\"{}\"", escape_toml_string(t)))
            .collect();
        lines.push(format!("tools    = [{}]", items.join(", ")));
    }

    // context_cost_tokens always 0
    lines.push("context_cost_tokens = 0".to_owned());

    // update
    if let Some(ch) = &args.update_channel {
        lines.push(format!(
            "update   = {{ channel = \"{}\", policy = \"pr\" }}",
            escape_toml_string(ch)
        ));
    }

    lines.join("\n")
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModuleKind;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!("sa_{}", nanos()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn nanos() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
    fn fixture(dir: &Path) {
        std::fs::write(dir.join("modules.toml"),
            "[registry.calibration]\nbytes_per_token = 0.813\nmeasured_at = \"2025-01-01T00:00:00Z\"\n").unwrap();
    }
    fn git_args(name: &str, cmd: &str) -> AddArgs {
        AddArgs {
            name: name.to_owned(),
            kind: crate::cli::KindArg::Mcp,
            git: Some("https://github.com/x/y".into()),
            rev: Some("abc123".into()),
            npm: None,
            npm_version: String::new(),
            cargo: None,
            cargo_version: String::new(),
            command: Some(cmd.to_owned()),
            tools: Vec::new(),
            risk: None,
            no_autoload: false,
            update_channel: None,
        }
    }

    #[test]
    fn valid_git_add() {
        let d = tmp();
        fixture(&d);
        run(&git_args("foo", "/run/foo"), &d).unwrap();
        let c = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(c.contains("[module.foo]"));
        assert!(c.contains("git"));
        assert!(c.contains("abc123"));
        let mf = ModulesFile::load(&d.join("modules.toml")).unwrap();
        assert!(mf.module.contains_key("foo"));
        assert_eq!(mf.module["foo"].kind, ModuleKind::Mcp);
    }

    #[test]
    fn valid_npm_add() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("my-npm", "/run");
        a.kind = crate::cli::KindArg::Skill;
        a.git = None;
        a.rev = None;
        a.command = None;
        a.npm = Some("@scope/pkg".into());
        a.npm_version = "1.0.0".into();
        run(&a, &d).unwrap();
        let c = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(c.contains("[module.my-npm]"));
        assert!(c.contains("npm"));
    }

    #[test]
    fn valid_cargo_add() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("my-crate", "/run");
        a.git = None;
        a.rev = None;
        a.cargo = Some("my-crate".into());
        a.cargo_version = "2.0.0".into();
        run(&a, &d).unwrap();
        let mf = ModulesFile::load(&d.join("modules.toml")).unwrap();
        assert!(mf.module.contains_key("my-crate"));
    }

    #[test]
    fn append_preserves_existing() {
        let d = tmp();
        let path = d.join("modules.toml");
        std::fs::write(&path, "# comment\n[registry.calibration]\nbytes_per_token = 0.813\nmeasured_at = \"2025-01-01T00:00:00Z\"\n\n[module.existing]\nkind = \"mcp\"\nsource = { git = \"https://x/y\", rev = \"abc\" }\ncommand = \"/run\"\n").unwrap();
        run(&git_args("new", "/run/new"), &d).unwrap();
        let c = std::fs::read_to_string(&path).unwrap();
        assert!(c.contains("# comment"));
        assert!(c.contains("[module.existing]"));
        assert!(c.contains("[module.new]"));
        let mf = ModulesFile::load(&path).unwrap();
        assert!(mf.module.contains_key("existing"));
        assert!(mf.module.contains_key("new"));
    }

    #[test]
    fn bad_name_rejected() {
        let d = tmp();
        fixture(&d);
        assert!(run(&git_args("Foo", ""), &d).is_err());
        assert!(run(&git_args("-x", ""), &d).is_err());
        assert!(run(&git_args("a b", ""), &d).is_err());
        assert!(run(&git_args("", ""), &d).is_err());
    }

    #[test]
    fn no_source_err() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("x", "/run");
        a.git = None;
        a.rev = None;
        assert!(run(&a, &d).is_err());
    }

    #[test]
    fn two_sources_err() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("x", "/run");
        a.npm = Some("pkg".into());
        assert!(run(&a, &d).is_err());
    }

    #[test]
    fn mcp_needs_command() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("x", "");
        a.command = None;
        assert!(run(&a, &d).is_err());
    }

    #[test]
    fn skill_no_command_ok() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("x", "");
        a.kind = crate::cli::KindArg::Skill;
        a.command = None;
        assert!(run(&a, &d).is_ok());
    }

    #[test]
    fn skill_with_command_err() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("x", "/run");
        a.kind = crate::cli::KindArg::Skill;
        assert!(run(&a, &d).is_err());
    }

    #[test]
    fn duplicate_err() {
        let d = tmp();
        fixture(&d);
        run(&git_args("dup", "/run/dup"), &d).unwrap();
        let before = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(run(&git_args("dup", "/run/dup2"), &d).is_err());
        assert_eq!(
            before,
            std::fs::read_to_string(d.join("modules.toml")).unwrap()
        );
    }

    #[test]
    fn autoload_and_tools_and_ctx() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("t", "/run");
        a.no_autoload = true;
        a.tools = vec!["read".into(), "write".into()];
        run(&a, &d).unwrap();
        let c = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(c.contains("autoload = false"));
        assert!(c.contains("tools"));
        assert!(c.contains("\"read\""));
        assert!(c.contains("\"write\""));
        assert!(c.contains("context_cost_tokens = 0"));
    }

    #[test]
    fn update_channel_present() {
        let d = tmp();
        fixture(&d);
        let mut a = git_args("u", "/run");
        a.update_channel = Some("tag".into());
        run(&a, &d).unwrap();
        let c = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(c.contains("update   = {"));
        assert!(c.contains("channel"));
        assert!(c.contains("tag"));
    }

    #[test]
    fn no_update_channel_absent() {
        let d = tmp();
        fixture(&d);
        run(&git_args("n", "/run"), &d).unwrap();
        let c = std::fs::read_to_string(d.join("modules.toml")).unwrap();
        assert!(!c.contains("update   = {"));
    }
}
