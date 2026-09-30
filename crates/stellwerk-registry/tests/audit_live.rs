//! Сквозная приёмка `stellwerk module audit`: настоящий бинарь, настоящий дочерний
//! процесс-сервер, настоящий stdio-кадр, настоящий код возврата.
//!
//! `tests/audit_transport.rs` (packet-09b) гоняет тот же транспорт в памяти, а unit-тесты
//! `cmd_audit` не знают про CLI. Здесь ровно то, чего нет в обоих: спавн, отчёт и код
//! возврата. Фикстура «MCP server» — sh-скрипт в tempdir: ни третьего файла в репозитории,
//! ни новой зависимости.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::OnceLock;

use stellwerk_registry::cli::{audit, CliAuditError, NameArgs};
use stellwerk_registry::cmd_audit::AuditError;

/// Тестовый бюджет: 50 мс вместо прод-10-секунд (packet-08), иначе таймаут не наблюдаем.
const TEST_BUDGET_MS: u64 = 50;

/// Манифест с одним mcp-модулем `fixture`; `__COMMAND__` и `__TOOLS__` подставляет тест.
const MODULES_TOML: &str = r#"
[registry.calibration]
tokens_per_byte = 0.2
measured_at = "2026-01-01T00:00:00Z"

[module.fixture]
kind = "mcp"
source = { git = "https://example.com/fixture", rev = "v1" }
command = "__COMMAND__"
tools = [__TOOLS__]
"#;

/// Манифест с одним skill-модулем: сервера у него нет и быть не должно.
const SKILL_MODULES_TOML: &str = r#"
[registry.calibration]
tokens_per_byte = 0.2
measured_at = "2026-01-01T00:00:00Z"

[module.notes]
kind = "skill"
source = { npm = "@scope/notes", version = "1.0.0" }
"#;

/// sh-«MCP server»: newline-delimited JSON-RPC на stdio, `id` эхом из строки запроса.
/// `__TOOLS__` — JSON-массив тулов, `__CALL__` — ответ на `tools/call` (`: ` — молчание).
const SERVER_SH: &str = r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fixture","version":"0.0.1"}}}\n' "$id" ;;
    *'"method":"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[__TOOLS__]}}\n' "$id" ;;
    *'"method":"tools/call"'*) __CALL__ ;;
  esac
done
"#;

/// Молчун: держит stdin открытым и не отвечает ни на что. Это «сервер не ответил» — не то
/// же самое, что «сервер ответил, но тул не вызвался» (AGENTS.md, закон 6).
const SERVER_SH_SILENT: &str = r#"#!/bin/sh
while IFS= read -r line; do
  :
done
"#;

/// Пишет свой PID в `__PIDFILE__` и выходит с кодом 3, не ответив на `initialize`. Файл нужен,
/// чтобы отличить reap от зомби: у зомби запись в `/proc` есть, у reap-нутого процесса её нет.
/// Путь вписывается в тело скрипта, потому что `Rpc::spawn` зовёт `Command::new(command)` без
/// аргументов.
const SERVER_SH_CRASH: &str = r#"#!/bin/sh
echo $$ > '__PIDFILE__'
exit 3
"#;

/// Что умеет фикстура: `Full` обслуживает t1,t2 и вызовы; `Extra` — t1,t2,t9 и вызовы;
/// `CallMute` молчит только на `tools/call`; `Silent` не отвечает вовсе.
#[derive(Clone, Copy)]
enum Answer {
    Full,
    Extra,
    CallMute,
    Silent,
}

/// Имена тулов в виде TOML-массива — то, что объявляет манифест.
fn toml_names(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",")
}

/// Те же имена в виде JSON-объектов MCP: у каждого тула обязателен `inputSchema`.
fn tools_list_json(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!(r#"{{"name":"{name}","inputSchema":{{"type":"object"}}}}"#))
        .collect::<Vec<_>>()
        .join(",")
}

/// Каталог фикстуры на одну сессию. В имя входит pid: два параллельных прогона (две копии
/// репозитория) не должны поделить один tempdir.
fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stellwerk_audit_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir
}

/// Каталог со всеми вариантами фикстурного сервера. Скрипты пишутся ОДИН раз на прогон,
/// и пишет их ОТДЕЛЬНЫЙ поток: execve файла, только что записанного тем же потоком, на
/// tmpfs возвращает ETXTBSY в ~3% случаев (замерено: 2109/64000 при задержке 0 мкс,
/// 12/64000 при 1 мс), а exec из другого потока после записи — 0/1500. Поэтому запись
/// и exec разнесены по потокам: ни один тест не exec'ит скрипт, который сам же записал.
fn scripts_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("stellwerk_audit_scripts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scripts dir");
        let dir2 = dir.clone();
        let writer = std::thread::spawn(move || {
            let call = r#"printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[]}}\n' "$id""#;
            let variants = [
                (
                    "full.sh",
                    SERVER_SH
                        .replace("__TOOLS__", &tools_list_json(&["t1", "t2"]))
                        .replace("__CALL__", call),
                ),
                (
                    "extra.sh",
                    SERVER_SH
                        .replace("__TOOLS__", &tools_list_json(&["t1", "t2", "t9"]))
                        .replace("__CALL__", call),
                ),
                (
                    "mute.sh",
                    SERVER_SH
                        .replace("__TOOLS__", &tools_list_json(&["t1"]))
                        .replace("__CALL__", ":"),
                ),
                ("silent.sh", SERVER_SH_SILENT.to_owned()),
            ];
            for (name, script) in variants {
                let p = dir2.join(name);
                std::fs::write(&p, script).expect("write the fixture server");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perm = std::fs::metadata(&p).expect("stat").permissions();
                    perm.set_mode(0o755);
                    std::fs::set_permissions(&p, perm).expect("chmod +x");
                }
            }
        });
        writer.join().expect("fixture writer");
        dir
    })
}

fn server_path(answer: Answer) -> PathBuf {
    scripts_dir().join(match answer {
        Answer::Full => "full.sh",
        Answer::Extra => "extra.sh",
        Answer::CallMute => "mute.sh",
        Answer::Silent => "silent.sh",
    })
}

/// Записать `modules.toml` для одного mcp-модуля `fixture`.
fn write_manifest(dir: &Path, command: &str, declared: &[&str]) {
    let manifest = MODULES_TOML
        .replace("__COMMAND__", command)
        .replace("__TOOLS__", &toml_names(declared));
    std::fs::write(dir.join("modules.toml"), manifest).expect("write modules.toml");
}

/// Прогнать собранный бинарь в каталоге фикстуры: его корень — cwd, ровно как в проде.
fn run_cli(root: &Path, module: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_stellwerk"))
        .current_dir(root)
        .args(["module", "audit", module])
        .output()
        .expect("run the built binary")
}

/// Тот же путь аудита внутри теста, но с инжектированным бюджетом: прод-дефолт в 10 секунд
/// для таймаут-случая слишком медленен.
fn audit_in_process(root: &Path, module: &str) -> Result<(String, i32), CliAuditError> {
    audit(
        &NameArgs {
            name: Some(module.to_owned()),
        },
        root,
        TEST_BUDGET_MS,
    )
}

#[test]
fn manifest_matching_the_server_exits_zero_with_ok() {
    let dir = unique_dir("match");
    let server = server_path(Answer::Full);
    write_manifest(&dir, server.to_str().expect("utf8 path"), &["t1", "t2"]);

    let out = run_cli(&dir, "fixture");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(0),
        "exact match must exit 0: {out:?}"
    );
    assert!(stdout.contains("OK"), "empty report says OK: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn extra_tool_in_server_exits_one_and_names_it() {
    let dir = unique_dir("extra");
    let server = server_path(Answer::Extra);
    write_manifest(&dir, server.to_str().expect("utf8 path"), &["t1", "t2"]);

    let out = run_cli(&dir, "fixture");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(1),
        "extra tool must exit 1: {out:?}"
    );
    assert!(
        stdout.contains("extra_in_server"),
        "list is named: {stdout}"
    );
    assert!(stdout.contains("t9"), "the extra tool is named: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_the_server_never_calls_is_not_invocable_and_exits_one() {
    let dir = unique_dir("mute_call");
    let server = server_path(Answer::CallMute);
    write_manifest(&dir, server.to_str().expect("utf8 path"), &["t1"]);

    let (report, code) =
        audit_in_process(&dir, "fixture").expect("silent call is a report, not an error");

    assert_eq!(code, 1, "an unanswerable tool must exit 1");
    assert!(report.contains("not_invocable"), "list is named: {report}");
    assert!(
        report.contains("t1"),
        "the unanswerable tool is named: {report}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn server_that_never_answers_is_not_reported_as_a_tool_verdict() {
    let dir = unique_dir("silent");
    let server = server_path(Answer::Silent);
    write_manifest(&dir, server.to_str().expect("utf8 path"), &["t1"]);

    let err = audit_in_process(&dir, "fixture").expect_err("a silent server yields no report");

    // Диагноз обязан называть «сервер не ответил», а не прятать это в verdict по туллам.
    assert!(
        matches!(err, CliAuditError::Audit(AuditError::NoAnswer(_))),
        "got {err:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn absent_command_exits_one_without_panicking() {
    let dir = unique_dir("absent");
    let absent = dir.join("absent-server.sh");
    write_manifest(&dir, absent.to_str().expect("utf8 path"), &["t1"]);

    let out = run_cli(&dir, "fixture");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_eq!(out.status.code(), Some(1), "no server is exit 1: {out:?}");
    assert!(stderr.contains("spawn"), "the cause is named: {stderr}");
    assert!(!stderr.contains("panicked"), "no panic: {stderr}");
    assert!(!stdout.contains("OK"), "no empty OK on failure: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skill_module_is_deferred_and_never_reads_as_a_pass() {
    let dir = unique_dir("skill");
    std::fs::write(dir.join("modules.toml"), SKILL_MODULES_TOML).expect("write modules.toml");

    let out = run_cli(&dir, "notes");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(0),
        "a deferral is not a failure: {out:?}"
    );
    assert!(
        stdout.contains("SKIPPED"),
        "deferral is its own line: {stdout}"
    );
    assert!(
        !stdout.contains("OK"),
        "a skipped audit must not read as a passed one: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// F-68: отказ на handshake обязан reap-ить дочерний процесс.
///
/// Проверяем не «произошла ошибка» — это уже покрывает тест на молчун, — а исчезновение PID.
/// `Child` в std при drop не делает ни kill, ни wait, поэтому без `impl Drop` процесс остаётся
/// зомби до конца жизни теста. 25 повторов отделяют «всегда» от «почти всегда».
#[test]
fn handshake_failure_reaps_the_child_process() {
    const ROUNDS: usize = 25;
    let dir = unique_dir("reap");
    let pidfile = dir.join("child.pid");
    let script = dir.join("crash.sh");
    let body = SERVER_SH_CRASH.replace("__PIDFILE__", pidfile.to_str().expect("utf8 pidfile"));

    // Запись в отдельном потоке: execve только что записанного скрипта на tmpfs возвращает
    // ETXTBSY примерно в 3% случаев, если писатель и exec'ющий — один поток (см. scripts_dir).
    let written = script.clone();
    let writer = std::thread::spawn(move || {
        std::fs::write(&written, body).expect("write the crashing server");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&written).expect("stat").permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&written, perm).expect("chmod +x");
        }
    });
    writer.join().expect("fixture writer");

    write_manifest(&dir, script.to_str().expect("utf8 path"), &["t1"]);

    let mut pids = Vec::with_capacity(ROUNDS);
    for _ in 0..ROUNDS {
        // Ошибка — ожидаемый исход: сервер умер, аудит обязан это увидеть, а не зависнуть.
        let _ = audit_in_process(&dir, "fixture");
        let raw = std::fs::read_to_string(&pidfile).expect("the fixture records its pid");
        pids.push(raw.trim().parse::<u32>().expect("numeric pid"));
    }

    let leaked: Vec<String> = pids
        .iter()
        .map(|pid| format!("/proc/{pid}"))
        .filter(|entry| Path::new(entry).exists())
        .map(|entry| {
            let stat = std::fs::read_to_string(Path::new(&entry).join("stat")).unwrap_or_default();
            let state = stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .unwrap_or("?");
            format!("{entry} state={state}")
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "{} из {} дочерних процессов не reap-нуты: {leaked:?}",
        leaked.len(),
        ROUNDS
    );
    let _ = std::fs::remove_dir_all(&dir);
}
