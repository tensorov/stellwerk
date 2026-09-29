//! RED-спецификация песочницы: внутри да, `..`-выход нет, корень напечатан скриптом,
//! программа и аргументы идут ВНУТРИ; отказ скрипта — `Failed` со своим кодом и `stderr`,
//! отсутствие скрипта — `Spawn`, иначе «нет данных» станет «выдуманной причиной» (закон 6).
//! `sandbox.sh` тянет `git archive` по всему репо, поэтому подмена идёт через
//! `STELLWERK_SANDBOX` под `ENV_LOCK`, а импорт — публичный API, не `#[path]` (F-14).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use stellwerk_task::sandbox::{Sandbox, SandboxError};

static ENV_LOCK: Mutex<()> = Mutex::new(());
static NEXT: AtomicU32 = AtomicU32::new(0);

#[test]
fn boundary_holds_and_runs_inside_with_arguments() {
    let fake = Fake::creating();
    let sb = prepare(&fake, "deadbeef").expect("подделка обязана собрать песочницу");
    let root = &fake.root;
    let root_text = fake.root.to_string_lossy();
    let escape = root.join("../выбрались");

    assert_eq!(sb.root(), root.as_path(), "корень напечатан скриптом");
    assert!(sb.contains_path(&root.join("src/lib.rs")));
    assert!(!sb.contains_path(&escape), "`..`-выход — false");
    assert!(!sb.contains_path(root.parent().unwrap()), "каталог снаружи");

    let args = ["-c", r#"pwd; printf ' %s' "$1""#, "sh", "аргумент"];
    let out = sb.command_in("sh", &args).output().expect("запуск");
    let said = String::from_utf8_lossy(&out.stdout).trim().to_string();

    assert!(out.status.success(), "программа отработала");
    assert!(said.starts_with(&*root_text), "каталог программы: {said:?}");
    assert!(said.ends_with(" аргумент"), "аргументы дойдут: {said:?}");
}

#[test]
fn refusing_script_is_failed_with_its_own_words() {
    let refused = prepare(&Fake::refusing(), "deadbeef").expect_err("exit 4 — это Err");

    match refused {
        SandboxError::Failed { status, stderr } => {
            assert_eq!(status.code(), Some(4), "код дошёл");
            assert!(stderr.contains("BLOCKED"), "причина: {stderr:?}");
        }
        other => panic!("отказавший скрипт — это Failed: {other:?}"),
    }
}

#[test]
fn missing_script_is_spawn_and_not_failure() {
    let fake = Fake::missing();
    let refused = prepare(&fake, "deadbeef").expect_err("нечего запускать — Err");

    match refused {
        SandboxError::Spawn { program, .. } => {
            let named = fake.script.to_string_lossy();
            assert_eq!(program, named, "назван непрошеный скрипт")
        }
        other => panic!("нет скрипта — это Spawn, а не Failed: {other:?}"),
    }
}

/// Подделка `sandbox.sh`: скрипт сам создаёт `root` и печатает его — контракт `sandbox_create`.
struct Fake {
    dir: PathBuf,
    script: PathBuf,
    root: PathBuf,
}

impl Fake {
    fn creating() -> Self {
        Self::new("ok", "mkdir -p '{root}'; printf '%s' '{root}'")
    }

    fn refusing() -> Self {
        Self::new("refused", "echo BLOCKED >&2; exit 4")
    }

    fn missing() -> Self {
        let mut fake = Self::new("gone", "");
        fake.script = fake.dir.join("нет-такого-скрипта.sh");
        fake
    }

    /// Пишется через `sh -c` и делается исполняемой: её обязаны писать и запускать как
    /// настоящий скрипт, иначе тест проверяет сам себя. `{root}` — каталог, который скрипт
    /// создаёт; имя уникально на запуск: `tempfile` в крейте нет.
    fn new(kind: &str, template: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("sw-{kind}-{pid}-{n}"));
        fs::create_dir_all(&dir).expect("временный каталог создаётся");
        let root = dir.join("box");
        let script = dir.join("sandbox.sh");
        let body = template.replace("{root}", &root.display().to_string());
        let sh = ["-c", "printf '%s' \"$1\" > \"$2\"; chmod +x \"$2\"", "sh"];
        let mut writer = Command::new("sh");
        let written = writer.args(sh).arg(body).arg(&script).status();
        assert!(written.expect("sh -c").success(), "подделка создана");
        Self { dir, script, root }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn prepare(fake: &Fake, base_commit: &str) -> Result<Sandbox, SandboxError> {
    let _serialized = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = std::env::var_os("STELLWERK_SANDBOX");
    std::env::set_var("STELLWERK_SANDBOX", &fake.script);
    let prepared = Sandbox::prepare(base_commit);
    match before {
        Some(was) => std::env::set_var("STELLWERK_SANDBOX", was),
        None => std::env::remove_var("STELLWERK_SANDBOX"),
    }
    prepared
}
