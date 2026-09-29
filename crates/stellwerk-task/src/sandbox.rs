//! # Адаптер песочницы
//!
//! Своя изоляция ЗАПРЕЩЕНА: `scripts/worker/sandbox.sh` делает `git archive` в `/tmp` без
//! `.git` (docs/PIVOT.md, F1) — обходится своя копия. Отказы различимы: «запускать нечего» и
//! «песочница сказала нет» требуют разной реакции, а молчаливое слияние превращает
//! отсутствие данных в выдуманную причину (закон 6). RED M2/packet-07: тела — `unimplemented!()`
//! (M2/packet-08); путь к скрипту из `STELLWERK_SANDBOX`, иначе тест зовёт `git archive`.

use std::path::{Path, PathBuf};
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
    pub fn prepare(base_commit: &str) -> Result<Sandbox, SandboxError> {
        let _ = base_commit;
        unimplemented!("Sandbox::prepare: реализация в M2/packet-08")
    }

    /// Команда ВНУТРИ песочницы: `current_dir` — корень. Непрочитанная: иначе отказ
    /// программы неотличим от отказа песочницы.
    pub fn command_in(&self, program: &str, args: &[&str]) -> Command {
        let _ = (self, program, args);
        unimplemented!("Sandbox::command_in: реализация в M2/packet-08")
    }

    /// Внутри ли песочницы путь `p`. `..`-выход — `false` и для несуществующего пути:
    /// `canonicalize` его уронил бы, а молчаливый `true` отдал бы запись снаружи.
    pub fn contains_path(&self, p: &Path) -> bool {
        let _ = (self, p);
        unimplemented!("Sandbox::contains_path: реализация в M2/packet-08")
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
