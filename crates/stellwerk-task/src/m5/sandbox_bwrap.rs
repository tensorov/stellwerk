//! # Песочница `bwrap`: границы проверяются до запуска, изоляцию даёт ядро
//!
//! Свою изоляцию не изобретаем: `bwrap` поднимает user namespace и монтирует корень
//! заново, поэтому root не нужен — машина клиентская, и «дать права» там невозможно в
//! принципе. Проверка границы живёт здесь и живёт ДО запуска: когда процесс уже напечатал
//! первую строку, запись снаружи состоялась, и отказ «по факту» не защищает ничего.
//!
//! Модуль СОБИРАЕТ и ОТВЕРГАЕТ, а запускает движок (`EngineAdapter::session`): песочнице
//! не надо знать, как именно убивают группу процессов, иначе таймаут исполнения
//! появился бы здесь вторым. Конфигурацию целиком даёт вызывающий: переменная окружения
//! сделала бы один и тот же запуск разным в разных терминалах.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use thiserror::Error;

/// Отказ песочницы, и оба варианта — ДО запуска. «Не прошла проверка» и «бюджет не помещается
/// в окно» не имеют ничего общего с «движок не поднялся»: чинятся они по-разному, и молчаливое
/// слияние отдало бы «нет данных» под видом отказа.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// Путь ушёл из рабочей копии после нормализации `..`.
    #[error("`{path}` leaves the working copy `{root}`: refused before launch")]
    Outside { root: PathBuf, path: PathBuf },
    /// Бюджет исполнения не помещается в окно песочницы: команда заведомо переживёт
    /// песочницу, и убивать её пришлось бы уже с потерей частичных данных.
    #[error("execution budget {budget:?} does not fit the sandbox window {window:?}: refused before launch")]
    Unfit { window: Duration, budget: Duration },
}

/// Песочница целиком в руках вызывающего: корень рабочей копии, отдельный каталог временных
/// файлов, системные каталоги на чтение и окно жизни. Ни одна переменная окружения тут не
/// читается: один и тот же запуск обязан давать один и тот же результат.
#[derive(Debug, Clone)]
pub struct Sandbox {
    /// Единственный каталог, куда команда пишет.
    pub work: PathBuf,
    /// Отдельный от рабочей копии: общий `/tmp` у двух песочниц — это гонка за один файл.
    pub tmp: PathBuf,
    /// Системные каталоги, только на чтение: агент читает конфиг и не переписывает его.
    pub read_only: Vec<PathBuf>,
    /// Окно песочницы: верхняя граница для всей сессии. Не бюджет исполнения.
    pub window: Duration,
}

impl Sandbox {
    /// Вектор аргументов `bwrap` — список, а не строка: shell-строкой нельзя ни доказать
    /// отсутствие интерпретации, ни передать путь с пробелом. `--unshare-net` — единственное
    /// место, где «сети нет» выражено; остальные флаги — про namespace'ы и про то, что прогон
    /// не переживает песочницу.
    pub fn argv(&self, program: &Path, args: &[&str]) -> Vec<OsString> {
        let mut argv: Vec<OsString> = [
            "--unshare-user",    // без него нужен root, а машина клиентская
            "--unshare-pid",     // прогон не видит процессов машины и не видит их потомков
            "--unshare-ipc",     // очереди сообщений не переживают границу
            "--unshare-uts",     // имя хоста не указывает на машину
            "--unshare-net",     // сети нет по умолчанию, а не «если её не включили»
            "--die-with-parent", // прогон умирает вместе с песочницей, а не после неё
            "--new-session",     // управляющего терминала у песочницы нет
            "--proc",
            "/proc",
            "--dev",
            "/dev",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        for dir in &self.read_only {
            Self::bind(&mut argv, "--ro-bind", dir);
        }
        // Записываются ровно два каталога, и оба свои: `--bind` на путь вне песочницы
        // отдал бы наружу кусок хостовой ФС, а это ровно то, чего песочница не делает.
        Self::bind(&mut argv, "--bind", &self.work);
        Self::bind(&mut argv, "--bind", &self.tmp);
        argv.push("--chdir".into());
        argv.push(self.work.as_os_str().into());
        argv.push("--".into());
        argv.push(program.into());
        argv.extend(args.iter().map(OsString::from));
        argv
    }

    /// `SRC` и `DEST` — один и тот же путь: каталог виден внутри по тому же пути, что и
    /// снаружи. Порядок важен: у `bwrap` флаг съедает ровно два следующих аргумента, и bind
    /// без адресата притворяется успешным, а монтирует не туда.
    fn bind(argv: &mut Vec<OsString>, flag: &str, dir: &Path) {
        argv.push(flag.into());
        argv.push(dir.into());
        argv.push(dir.into());
    }

    /// Команда для запуска внутри песочницы. `write` — каталог, которому команда адресована
    /// для записи; `budget` — бюджет исполнения, который обязан помещаться в окно.
    pub fn command(
        &self,
        write: &Path,
        program: &Path,
        args: &[&str],
        budget: Duration,
    ) -> Result<Command, SandboxError> {
        self.preflight(write, budget)?;
        let mut command = Command::new("bwrap");
        command.args(self.argv(program, args));
        Ok(command)
    }

    /// Оба отказа — до запуска, и оба — про саму песочницу. Их порядок не важен: каждый
    /// отказ называет свой виновник, и ни один не значит «запуск не удался».
    fn preflight(&self, write: &Path, budget: Duration) -> Result<(), SandboxError> {
        if budget > self.window {
            return Err(SandboxError::Unfit {
                window: self.window,
                budget,
            });
        }
        let (root, path) = (normalize(&self.work), normalize(write));
        if path.starts_with(&root) {
            Ok(())
        } else {
            Err(SandboxError::Outside { root, path })
        }
    }
}

/// Нормализация лексическая: `..` и `.` решаются руками, без ФС. `canonicalize` уронил бы
/// несуществующий путь, а решить «по существованию» нельзя — тогда проверка границы врала
/// бы ровно там, где опасно. Вверх за `/` не поднимаемся: нечего отдавать.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
