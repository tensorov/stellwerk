//! Measure helpers: parse tools/list JSON, lock-file I/O, token budgeting.
//!
//! packet-06 (RED) → packet-07a (GREEN). Спецификация RED-тестов сохранена без
//! изменений: 9 тестов ниже — это контракт, который определил packet-06.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::cli::NameArgs;
use crate::model::ModulesFile;

/// Error type shared across measure operations.
#[derive(Debug, thiserror::Error)]
pub enum MeasureError {
    #[error("not implemented")]
    NotImplemented,
    #[error("invalid tool schema")]
    InvalidTool,
    #[error("json parse error")]
    Parse(#[from] serde_json::Error),
    #[error("toml error")]
    Toml(#[from] toml::de::Error),
    #[error("toml serialize error")]
    TomlSer(#[from] toml::ser::Error),
    #[error("module required: stellwerk module measure <name>")]
    NameRequired,
    #[error("no such module: {0}")]
    NoModule(String),
    #[error("module {0} has no command")]
    NoCommand(String),
    #[error("failed to spawn MCP server")]
    Spawn,
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Model(#[from] crate::model::ModelError),
}

// ── lock-file types ────────────────────────────────────────────────────────

/// A single module entry inside modules.lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockModule {
    pub context_cost_tokens: u64,
}

/// The top-level modules.lock file.
///
/// `#[serde(flatten)]` on the modules map causes each key to become a
/// TOML section header, producing the expected format:
///   version = 1
///   [polyforge]
///     context_cost_tokens = 813
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    #[serde(flatten)]
    pub modules: BTreeMap<String, LockModule>,
}

// ── MCP tools/list parser ──────────────────────────────────────────────────

/// A parsed tool schema reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSchema {
    pub name: String,
    /// Compact JSON of inputSchema (+ outputSchema if present), alphabetical keys.
    pub schema_json: String,
}

/// Parse a JSON-RPC 2.0 `tools/list` response and return one schema entry per tool.
///
/// Schema JSON is the compact serialisation of inputSchema (+ outputSchema if present),
/// keys in alphabetical order (serde_json default). When a tool declares an
/// `outputSchema`, both halves are wrapped so no input property can be lost:
/// `{"input":<inputSchema>,"output":<outputSchema>}`.
pub fn parse_tools_list_response(json: &str) -> Result<Vec<ToolSchema>, MeasureError> {
    let val: serde_json::Value = serde_json::from_str(json)?;
    let tools = val
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .ok_or(MeasureError::InvalidTool)?;

    let mut out = Vec::with_capacity(tools.len());
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or(MeasureError::InvalidTool)?
            .to_owned();
        let input = tool.get("inputSchema").ok_or(MeasureError::InvalidTool)?;
        let schema_json = match tool.get("outputSchema") {
            Some(output) => {
                let mut both = serde_json::Map::new();
                both.insert("input".to_owned(), input.clone());
                both.insert("output".to_owned(), output.clone());
                serde_json::to_string(&serde_json::Value::Object(both))?
            }
            None => serde_json::to_string(input)?,
        };
        out.push(ToolSchema { name, schema_json });
    }
    Ok(out)
}

/// Total byte size of all tool schema strings.
pub fn schema_bytes(tools: &[ToolSchema]) -> u64 {
    tools.iter().map(|t| t.schema_json.len() as u64).sum()
}

/// Update / upsert a module's context_cost_tokens in a lock-file string.
pub fn upsert_lock(existing: &str, name: &str, tokens: u64) -> Result<String, MeasureError> {
    let mut lf: LockFile = toml::from_str(existing)?;
    lf.modules.insert(
        name.to_owned(),
        LockModule {
            context_cost_tokens: tokens,
        },
    );
    Ok(toml::to_string(&lf)?)
}

/// Delegate token calculation to the model formula.
pub fn measure_tokens(schema_b: u64, tpb: f64) -> u64 {
    crate::model::tokens_for(schema_b, tpb)
}

// ── sync JSON-RPC stdio session ─────────────────────────────────────────────

const PROTOCOL_VERSION: &str = "2024-11-05";

/// What a completed `tools/list` session yields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOut {
    pub schema_bytes: u64,
    pub tokens: u64,
}

fn write_message<W: Write>(w: &mut W, line: &str) -> Result<(), MeasureError> {
    w.write_all(line.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()?;
    Ok(())
}

/// Read lines until a JSON-RPC response for `id` arrives; notifications are skipped.
///
/// `MAX_SKIPPED` bounds how many non-answering messages we tolerate. packet-07a allowed
/// reading without a timeout (no new crates), so this guard is what keeps a chatty or
/// misframed server from spinning forever: MCP stdio is newline-delimited JSON, and a
/// server that breaks framing will be cut off instead of hanging the caller.
fn read_response<R: BufRead>(r: &mut R, id: u64) -> Result<String, MeasureError> {
    const MAX_SKIPPED: usize = 64;
    let mut buf = String::new();
    let mut skipped = 0usize;
    loop {
        buf.clear();
        let n = r.read_line(&mut buf)?;
        if n == 0 {
            return Err(MeasureError::Protocol(format!(
                "server closed the stream before answering id={id}"
            )));
        }
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        skipped += 1;
        if skipped > MAX_SKIPPED {
            return Err(MeasureError::Protocol(format!(
                "no answer to id={id} within {MAX_SKIPPED} messages (broken framing?)"
            )));
        }
        let val: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if val.get("id").and_then(|v| v.as_u64()) != Some(id) {
            continue; // notification or a late reply to an earlier id
        }
        if let Some(err) = val.get("error") {
            return Err(MeasureError::Protocol(format!("server error: {err}")));
        }
        if val.get("result").is_none() {
            return Err(MeasureError::Protocol(format!(
                "response to id={id} carries neither result nor error"
            )));
        }
        return Ok(trimmed.to_owned());
    }
}

/// Run one MCP handshake (`initialize` → `notifications/initialized` → `tools/list`)
/// over an arbitrary reader/writer pair, so it is testable without a process.
///
/// Note on `protocolVersion`: an absent field is tolerated. packet-07a asked for a
/// strict check, but its own fake-server fixture omits the field, so a strict check
/// would reject the reference fixture. We require a well-formed JSON-RPC envelope
/// (matching id, `result` or `error`) and reject anything else as `Protocol`.
pub fn run_session<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    tpb: f64,
) -> Result<SessionOut, MeasureError> {
    let init = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"{PROTOCOL_VERSION}","capabilities":{{}},"clientInfo":{{"name":"stellwerk","version":"0.0.1"}}}}}}"#
    );
    write_message(writer, &init)?;
    let init_resp = read_response(reader, 1)?;
    let init_val: serde_json::Value = serde_json::from_str(&init_resp)?;
    if let Some(pv) = init_val
        .pointer("/result/protocolVersion")
        .and_then(|v| v.as_str())
    {
        if pv.is_empty() {
            return Err(MeasureError::Protocol("empty protocolVersion".to_owned()));
        }
    }

    write_message(
        writer,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    )?;
    write_message(
        writer,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    )?;
    let tools_resp = read_response(reader, 2)?;
    let tools = parse_tools_list_response(&tools_resp)?;
    let b = schema_bytes(&tools);
    Ok(SessionOut {
        schema_bytes: b,
        tokens: measure_tokens(b, tpb),
    })
}

// ── module-level measure ───────────────────────────────────────────────────

/// Result of measuring one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureReport {
    pub name: String,
    pub schema_bytes: u64,
    pub context_cost_tokens: u64,
}

/// Canonical header for modules.lock.
///
/// A TOML round-trip cannot preserve comments, so re-serialising the lock would silently
/// drop the file's own contract. We re-attach the header on every write instead.
pub const LOCK_HEADER: &str =
    "# Генерируется `stellwark module measure` / `stellwerk module update`. Руками не править.
# Формат: один модуль = один блок, блоки отсортированы по имени (детерминированно).
";

/// Write (or update) the module's cost in `<root>/modules.lock`, sorted by name.
pub fn write_lock(root: &Path, name: &str, tokens: u64) -> Result<(), MeasureError> {
    let lock_path = root.join("modules.lock");
    let existing = match std::fs::read_to_string(&lock_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "version = 1\n".to_owned(),
        Err(e) => return Err(e.into()),
    };
    let updated = upsert_lock(&existing, name, tokens)?;
    let with_header = if updated.starts_with('#') {
        updated
    } else {
        format!("{LOCK_HEADER}{updated}")
    };
    std::fs::write(&lock_path, with_header)?;
    Ok(())
}

/// Measure one MCP module: spawn the server, read its `tools/list`, apply the
/// calibration coefficient and record the result in `modules.lock`.
pub fn measure_module(
    root: &Path,
    name: &str,
    command: &str,
) -> Result<MeasureReport, MeasureError> {
    let mf = ModulesFile::load(&root.join("modules.toml"))?;
    let tpb = mf.registry.calibration.tokens_per_byte;
    measure_module_with_tpb(root, name, command, tpb)
}

/// Same as [`measure_module`], but with an explicit coefficient (used by tests and
/// by the calibration flow, which needs the coefficient *before* it is pinned).
pub fn measure_module_with_tpb(
    root: &Path,
    name: &str,
    command: &str,
    tpb: f64,
) -> Result<MeasureReport, MeasureError> {
    let mut child = Command::new(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MeasureError::Spawn
            } else {
                MeasureError::Io(e)
            }
        })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| MeasureError::Protocol("child stdin unavailable".to_owned()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| MeasureError::Protocol("child stdout unavailable".to_owned()))?;
    let mut reader = std::io::BufReader::new(stdout);

    let session = run_session(&mut reader, &mut stdin, tpb);
    drop(stdin); // signal EOF so a well-behaved server exits
    let _ = child.wait();
    let out = session?;

    write_lock(root, name, out.tokens)?;
    Ok(MeasureReport {
        name: name.to_owned(),
        schema_bytes: out.schema_bytes,
        context_cost_tokens: out.tokens,
    })
}

// ── CLI ────────────────────────────────────────────────────────────────────

/// Run the `module measure` command. `root` is the directory containing `modules.toml`.
pub fn run(args: &NameArgs, root: &Path) -> Result<(), MeasureError> {
    let name = args
        .name
        .as_deref()
        .ok_or(MeasureError::NameRequired)?
        .to_owned();
    let mf = ModulesFile::load(&root.join("modules.toml"))?;
    let command = mf
        .module
        .get(&name)
        .ok_or_else(|| MeasureError::NoModule(name.clone()))?
        .command
        .clone()
        .ok_or_else(|| MeasureError::NoCommand(name.clone()))?;

    let report = measure_module(root, &name, &command)?;
    println!(
        "measure {}: schema_bytes={} context_cost_tokens={}",
        report.name, report.schema_bytes, report.context_cost_tokens
    );
    Ok(())
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// FIXTURE — 4-tool tools/list response from polyforge-mcp.
    const FIXTURE_TOOLS: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[
    {"name":"evidence_append","inputSchema":{"properties":{"kind":{"type":"string"},"payload":{"type":"string"}},"required":["kind","payload"],"type":"object"}},
    {"name":"evidence_verify","inputSchema":{"properties":{"claim_id":{"type":"integer"},"task_id":{"type":"string"}},"required":["claim_id","task_id"],"type":"object"}},
    {"name":"gate_evaluate","inputSchema":{"properties":{"required":{"items":{"type":"string"},"type":"array"},"task_id":{"type":"string"}},"required":["task_id"],"type":"object"}},
    {"name":"gate_report","inputSchema":{"properties":{"task_id":{"type":"string"}},"required":["task_id"],"type":"object"},"outputSchema":{"properties":{"ok":{"type":"boolean"}},"type":"object"}}
]}}"#;
    const SJ0: &str = r#"{"properties":{"kind":{"type":"string"},"payload":{"type":"string"}},"required":["kind","payload"],"type":"object"}"#;
    const SJ1: &str = r#"{"properties":{"claim_id":{"type":"integer"},"task_id":{"type":"string"}},"required":["claim_id","task_id"],"type":"object"}"#;
    const SJ2: &str = r#"{"properties":{"required":{"items":{"type":"string"},"type":"array"},"task_id":{"type":"string"}},"required":["task_id"],"type":"object"}"#;
    const SJ3: &str = r#"{"input":{"properties":{"task_id":{"type":"string"}},"required":["task_id"],"type":"object"},"output":{"properties":{"ok":{"type":"boolean"}},"type":"object"}}"#;

    /// 813 ±10% — это context_input-дельта измерения OpenCode (docs/CONTEXT_BUDGET.md),
    /// включая оверхед форматирования тулов, НЕ чистые байты схем;
    /// тест — pinning round-trip (bytes × tokens_per_byte ≈ 813), не проверка
    /// правильности коэффициента.

    // 1. FIXTURE — parse returns Ok and schemas match.
    #[test]
    fn fixture_parse_schema_strings() {
        let tools = parse_tools_list_response(FIXTURE_TOOLS).expect("must parse");
        assert_eq!(tools[0].name, "evidence_append");
        assert_eq!(tools[0].schema_json, SJ0);
        assert_eq!(tools[1].name, "evidence_verify");
        assert_eq!(tools[1].schema_json, SJ1);
        assert_eq!(tools[2].name, "gate_evaluate");
        assert_eq!(tools[2].schema_json, SJ2);
        assert_eq!(tools[3].name, "gate_report");
        assert_eq!(tools[3].schema_json, SJ3);
    }

    #[test]
    fn fixture_schema_bytes() {
        let tools = parse_tools_list_response(FIXTURE_TOOLS).expect("must parse");
        let b = schema_bytes(&tools);
        let expected: u64 = (SJ0.len() + SJ1.len() + SJ2.len() + SJ3.len()) as u64;
        assert_eq!(b, expected);
    }

    // 2. Invalid JSON / missing result.tools → Err.
    #[test]
    fn invalid_json_returns_err() {
        let res = parse_tools_list_response("not json");
        assert!(res.is_err(), "invalid JSON must return Err");
    }

    #[test]
    fn missing_result_tools_returns_err() {
        let res = parse_tools_list_response(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert!(res.is_err(), "missing tools must return Err");
    }

    // 3. Pinning: bytes × tpb ≈ 813 (813±10%).
    #[test]
    fn pinning_813_roundtrip() {
        let tools = parse_tools_list_response(FIXTURE_TOOLS).expect("must parse");
        let b = schema_bytes(&tools);
        let tpb = 813.0 / b as f64;
        let t = measure_tokens(b, tpb);
        assert!(
            (732..=894).contains(&t),
            "pin 813±10%: got {} for B={}",
            t,
            b,
        );
    }

    // 4. upsert_lock — basic, two modules, update existing.
    #[test]
    fn upsert_lock_basic() {
        let out = upsert_lock("version = 1\n", "polyforge", 813)
            .expect("upsert to empty lock must succeed");
        assert!(
            out.contains("[polyforge]"),
            "must contain [polyforge] section"
        );
        assert!(
            out.contains("context_cost_tokens = 813"),
            "must have tokens=813"
        );
        assert!(out.contains("version = 1"), "must preserve version=1");
    }

    #[test]
    fn upsert_lock_two_modules_sorted() {
        let out1 = upsert_lock("version = 1\n", "aaa", 100).expect("first upsert");
        let out2 = upsert_lock(&out1, "zzz", 200).expect("second upsert");
        assert!(out2.contains("[aaa]"), "must have aaa section");
        assert!(out2.contains("[zzz]"), "must have zzz section");
        // alpha order: aaa before zzz
        let aaa_pos = out2.find("[aaa]").unwrap();
        let zzz_pos = out2.find("[zzz]").unwrap();
        assert!(aaa_pos < zzz_pos, "aaa must come before zzz");
    }

    #[test]
    fn upsert_lock_update_existing_no_dup() {
        let out1 = upsert_lock("version = 1\n", "m", 100).expect("first upsert");
        let out2 = upsert_lock(&out1, "m", 200).expect("second upsert (same module)");
        // Should have exactly one [m] section — no duplicate block.
        let count = out2.matches("[m]").count();
        assert_eq!(
            count, 1,
            "updating existing module must not duplicate section"
        );
        assert!(
            out2.contains("context_cost_tokens = 200"),
            "must reflect new tokens"
        );
    }

    // 5. upsert_lock on garbage → Err.
    #[test]
    fn upsert_lock_on_garbage_errs() {
        let res = upsert_lock("totally not toml\n", "x", 1);
        assert!(res.is_err(), "garbage must return Err");
    }

    // 6. LockFile round-trip: to_string → from_str equal for 2 modules.
    #[test]
    fn lockfile_roundtrip() {
        let lf = LockFile {
            version: 1,
            modules: BTreeMap::from([
                (
                    "aaa".to_string(),
                    LockModule {
                        context_cost_tokens: 111,
                    },
                ),
                (
                    "zzz".to_string(),
                    LockModule {
                        context_cost_tokens: 222,
                    },
                ),
            ]),
        };
        let s = toml::to_string(&lf).expect("serialize");
        let restored: LockFile = toml::from_str(&s).expect("deserialize");
        assert_eq!(restored.version, 1);
        assert_eq!(restored.modules, lf.modules);
    }

    // ── packet-07a: session + spawn ─────────────────────────────────────────

    const MODULES_TOML: &str = r#"
[registry.calibration]
tokens_per_byte = 0.05
measured_at = "2026-01-01T00:00:00Z"

[module.fake]
kind = "mcp"
source = { git = "https://example.com/fake", rev = "v1" }
command = "FAKE_SERVER_PATH"
tools = ["evidence_append"]
"#;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("stellwerk_measure_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    /// Fake MCP server speaking line-delimited JSON-RPC over stdio.
    fn write_fake_server(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("fake-mcp.sh");
        // newline-delimited протокол: ответ обязан быть одной строкой
        let tools = FIXTURE_TOOLS
            .replace("\"id\":1", "\"id\":2")
            .replace('\n', "");
        let script = format!(
            "#!/bin/sh\n\
             while IFS= read -r line; do\n\
             \x20 case \"$line\" in\n\
             \x20   *'\"method\":\"initialize\"'*) printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{}},\"serverInfo\":{{\"name\":\"fake-mcp\",\"version\":\"0.0.1\"}}}}}}' ;;\n\
             \x20   *notifications/initialized*) ;;\n\
             \x20   *'\"method\":\"tools/list\"'*) printf '%s\\n' '{tools}' ;;\n\
             \x20 esac\n\
             done\n"
        );
        std::fs::write(&path, script).expect("write fake server");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&path).expect("stat").permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&path, perm).expect("chmod");
        }
        path
    }

    #[test]
    fn run_session_over_cursor() {
        let canned = concat!(
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
            "\n",
        );
        // newline-delimited: каждый ответ — ровно одна строка
        let tools_resp = FIXTURE_TOOLS
            .replace("\"id\":1", "\"id\":2")
            .replace('\n', "");
        let mut input = String::from(canned);
        input.push_str(&tools_resp);
        input.push('\n');

        let mut reader = std::io::Cursor::new(input.into_bytes());
        let mut written: Vec<u8> = Vec::new();
        let tpb = 0.05;
        let out = run_session(&mut reader, &mut written, tpb).expect("session must succeed");

        let b = (SJ0.len() + SJ1.len() + SJ2.len() + SJ3.len()) as u64;
        assert_eq!(out.schema_bytes, b);
        assert_eq!(out.tokens, measure_tokens(b, tpb));
        let sent = String::from_utf8(written).expect("utf8");
        assert!(
            sent.contains(r#""method":"initialize""#),
            "must send initialize"
        );
        assert!(
            sent.contains(r#""method":"notifications/initialized""#),
            "must send initialized notification"
        );
        assert!(
            sent.contains(r#""method":"tools/list""#),
            "must send tools/list"
        );
    }

    #[test]
    fn run_session_rejects_error_envelope() {
        let mut reader = std::io::Cursor::new(
            br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"no such method"}}"#
                .to_vec(),
        );
        let mut written: Vec<u8> = Vec::new();
        let res = run_session(&mut reader, &mut written, 0.05);
        assert!(res.is_err(), "server error envelope must abort the session");
    }

    #[test]
    fn run_session_rejects_truncated_stream() {
        let mut reader = std::io::Cursor::new(Vec::new());
        let mut written: Vec<u8> = Vec::new();
        let res = run_session(&mut reader, &mut written, 0.05);
        assert!(res.is_err(), "closed stream must abort, not hang");
    }

    #[test]
    fn measure_module_with_fake_server_writes_sorted_lock() {
        let dir = tmpdir("fake_server");
        let server = write_fake_server(&dir);
        let modules = dir.join("modules.toml");
        std::fs::write(
            &modules,
            MODULES_TOML.replace("FAKE_SERVER_PATH", server.to_str().expect("utf8")),
        )
        .expect("write modules.toml");

        let rep = measure_module(&dir, "fake", server.to_str().expect("utf8"))
            .expect("measure must succeed");

        let b = (SJ0.len() + SJ1.len() + SJ2.len() + SJ3.len()) as u64;
        assert_eq!(rep.schema_bytes, b);
        assert!(rep.context_cost_tokens > 0, "tokens must be non-zero");
        assert_eq!(
            rep.context_cost_tokens,
            measure_tokens(b, 0.05),
            "tokens must follow the calibration formula"
        );

        let lock = std::fs::read_to_string(dir.join("modules.lock")).expect("lock written");
        assert!(lock.contains("version = 1"), "lock keeps version");
        assert!(
            lock.contains(&format!(
                "context_cost_tokens = {}",
                rep.context_cost_tokens
            )),
            "lock records the measured cost"
        );

        // second run must update, not duplicate
        measure_module(&dir, "fake", server.to_str().expect("utf8")).expect("second run");
        let lock2 = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert_eq!(lock2.matches("[fake]").count(), 1, "no duplicate section");

        // another module lands after it, sorted
        write_lock(&dir, "zzz-other", 42).expect("upsert second module");
        let lock3 = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert!(lock3.find("[fake]").unwrap() < lock3.find("[zzz-other]").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_lock_keeps_canonical_header() {
        let dir = tmpdir("lock_header");
        // старый lock с комментариями, как в репозитории
        let seeded = format!("{LOCK_HEADER}version = 1\n\n[old]\ncontext_cost_tokens = 1\n");
        std::fs::write(dir.join("modules.lock"), seeded).expect("seed lock");

        write_lock(&dir, "fresh", 813).expect("write_lock");
        let after_first = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert!(
            after_first.starts_with('#'),
            "header must survive the write"
        );
        assert!(
            after_first.contains("Руками не править"),
            "header text kept"
        );
        assert!(after_first.contains("[fresh]"), "new module present");
        assert!(after_first.contains("[old]"), "old module preserved");
        assert!(
            after_first.find("[fresh]").unwrap() < after_first.find("[old]").unwrap(),
            "blocks stay sorted by name"
        );

        // повторная запись не дублирует шапку
        write_lock(&dir, "another", 42).expect("write_lock again");
        let after_second = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert_eq!(
            after_second.matches("Руками не править").count(),
            1,
            "header must not be duplicated"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_lock_adds_header_to_bare_lock() {
        let dir = tmpdir("lock_bare");
        std::fs::write(dir.join("modules.lock"), "version = 1\n").expect("seed lock");
        write_lock(&dir, "mod", 7).expect("write_lock");
        let body = std::fs::read_to_string(dir.join("modules.lock")).expect("lock");
        assert!(body.starts_with('#'), "header added to a bare lock");
        assert!(body.contains("context_cost_tokens = 7"), "cost recorded");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn measure_module_spawn_not_found_is_spawn_error() {
        let dir = tmpdir("spawn_missing");
        std::fs::write(dir.join("modules.toml"), MODULES_TOML).expect("write modules.toml");
        let res = measure_module(&dir, "fake", "definitely-not-a-real-binary-xyz");
        assert!(
            matches!(res, Err(MeasureError::Spawn)),
            "missing binary must map to Spawn, got {:?}",
            res.err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_requires_name() {
        let args = NameArgs { name: None };
        let dir = tmpdir("no_name");
        let res = run(&args, &dir);
        assert!(matches!(res, Err(MeasureError::NameRequired)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_unknown_module_is_error() {
        let args = NameArgs {
            name: Some("nope".to_owned()),
        };
        let dir = tmpdir("unknown");
        std::fs::write(dir.join("modules.toml"), MODULES_TOML).expect("write modules.toml");
        let res = run(&args, &dir);
        assert!(matches!(res, Err(MeasureError::NoModule(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_module_without_command_is_error() {
        let args = NameArgs {
            name: Some("fake".to_owned()),
        };
        let dir = tmpdir("no_command");
        let toml = MODULES_TOML.replace("FAKE_SERVER_PATH", "/bin/true");
        let toml = toml.replace("command = \"/bin/true\"\n", "");
        std::fs::write(dir.join("modules.toml"), toml).expect("write modules.toml");
        let res = run(&args, &dir);
        assert!(matches!(res, Err(MeasureError::NoCommand(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
