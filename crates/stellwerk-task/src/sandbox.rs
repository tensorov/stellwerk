//! # Адаптер песочницы
//!
//! Своя изоляция ЗАПРЕЩЕНА: `scripts/worker/sandbox.sh` делает `git archive` в `/tmp` без
//! `.git` (docs/PIVOT.md, F1) — обходится своя копия. Отказы различимы: «запускать нечего» и
//! «песочница сказала нет» требуют разной реакции, а молчаливое слияние превращает
//! отсутствие данных в выдуманную причину (закон 6). RED M2/packet-07: тела — `unimplemented!()`
//! (M2/packet-08); путь к скрипту из `STELLWERK_SANDBOX`, иначе тест зовёт `git archive`.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Отказ песочницы.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// Запускать нечего: нет файла, нет прав. Не то же, что `Failed`.
    #[error("sandbox script `{program}` did not start: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    /// Скрипт отказал: его код и `stderr`. Пустой `stderr` — тоже данные.
    #[error("sandbox script refused ({status}): {stderr}")]
    Failed {
        status: std::process::ExitStatus,
        stderr: String,
    },
    /// Путь снаружи. Пока не выдаётся: `contains_path` отвечает `bool`.
    #[error("path leaves sandbox: {0}")]
    Outside(String),
}

/// Собранное окружение прогона.
#[derive(Debug)]
pub struct Sandbox {
    /// Корень напечатал скрипт в stdout: угадывать его нельзя.
    root: PathBuf,
    /// `SANDBOX_KEEP`: оставить песочницу после прогона; уборкой занят скрипт.
    keep: bool,
}

impl Sandbox {
    /// Собрать окружение прогона для `base_commit` скриптом песочницы.
    ///
    /// Скрипт запускается как `sh <script> <base_commit>`; путь к скрипту берётся из
    /// `STELLWERK_SANDBOX` **в момент вызова** (не кэшем при первом обращении: переменная
    /// принадлежит процессу, и кэш отдал бы второму прогону скрипт первого).
    pub fn prepare(base_commit: &str) -> Result<Sandbox, SandboxError> {
        let script =
            std::env::var_os("STELLWERK_SANDBOX").map_or_else(default_script, PathBuf::from);
        let program = script.to_string_lossy().into_owned();

        // Предполёт: скрипт должен существовать и читаться. Без него `sh` ответит
        // 127 «cannot open» — по коду это неотличимо от отказа песочницы, а чинить
        // такой отчёт нечем: искать политику там, где опечатка в пути (закон 6).
        if let Err(source) = fs::File::open(&script) {
            return Err(SandboxError::Spawn { program, source });
        }

        let run = Command::new("sh")
            .arg(&script)
            .arg(base_commit)
            .output()
            .map_err(|source| SandboxError::Spawn { program, source })?;

        if !run.status.success() {
            // Скрипт отказал своими словами: его код и его stderr — иначе отказ
            // без причины нельзя ни прочитать, ни починить.
            return Err(SandboxError::Failed {
                status: run.status,
                stderr: text(&run.stderr),
            });
        }

        let root = parse_root(&run.stdout).ok_or_else(|| SandboxError::Failed {
            status: run.status,
            stderr: format!(
                "песочница не напечатала корень (пустой stdout); stderr скрипта: {}",
                text(&run.stderr).trim_end()
            ),
        })?;

        Ok(Sandbox {
            root,
            keep: keep_requested(),
        })
    }

    /// Команда ВНУТРИ песочницы: `current_dir` — корень. Непрочитанная: иначе отказ
    /// программы неотличим от отказа песочницы.
    pub fn command_in(&self, program: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(&self.root).args(args);
        cmd
    }

    /// Внутри ли песочницы путь `p`. `..`-выход — `false` и для несуществующего пути:
    /// `canonicalize` его уронил бы, а молчаливый `true` отдал бы запись снаружи.
    pub fn contains_path(&self, p: &Path) -> bool {
        normalize(p).starts_with(normalize(&self.root))
    }

    /// Корень песочницы: без него путь внутри не собрать, а поля мертвы — и скрыть это
    /// можно лишь запрещённым `#[allow]`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Помечена ли песочница на сохранение после прогона.
    pub fn is_kept(&self) -> bool {
        self.keep
    }
}

/// Скрипт по умолчанию — тот, что в репозитории. Подменяется `STELLWERK_SANDBOX`:
/// тест не должен звать `git archive` по всему репо.
fn default_script() -> PathBuf {
    Path::new("scripts/worker/sandbox.sh").to_path_buf()
}

/// `SANDBOX_KEEP` — флаг скрипта: `1` — оставить песочницу, `0`/пусто/нет переменной —
/// удалить (scripts/worker/dispatch.conf:82). Уборкой занят скрипт, здесь только чтение.
fn keep_requested() -> bool {
    std::env::var("SANDBOX_KEEP").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Корень напечатан скриптом в stdout: угадывать его нельзя. Пусто — данных нет.
fn parse_root(stdout: &[u8]) -> Option<PathBuf> {
    let printed = text(stdout).trim().to_string();
    (!printed.is_empty()).then(|| PathBuf::from(printed))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Нормализация ЛЕКСИЧЕСКАЯ: `..` и `.` решаются руками, без ФС. `canonicalize` уронил бы
/// несуществующий путь, а решать «по существованию» нельзя — тогда проверка границы
/// врала бы именно там, где опасно. Вверх за `/` не поднимаемся: нечего отдавать.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
            Component::RootDir | Component::Prefix(_) => out.push(c.as_os_str()),
        }
    }
    out
}
