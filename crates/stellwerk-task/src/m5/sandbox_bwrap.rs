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
use std::os::unix::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use thiserror::Error;

/// Отказ песочницы, и все три варианта — ДО запуска. «Не прошла проверка», «это клон» и
/// «бюджет не помещается в окно» не имеют ничего общего с «движок не поднялся»: чинятся
/// они по-разному, и молчаливое слияние отдало бы «нет данных» под видом отказа.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// Путь ушёл из рабочей копии после нормализации `..`.
    #[error("`{path}` leaves the working copy `{root}`: refused before launch")]
    Outside { root: PathBuf, path: PathBuf },
    /// `.git` физически внутри: копия с историей — это клон, а не выгрузка состояния.
    #[error("`{path}` sits inside the working copy: history is not a state export")]
    History { path: PathBuf },
    /// Бюджет исполнения не помещается в окно песочницы: команда заведомо переживёт
    /// песочницу, и убивать её пришлось бы уже с потерей частичных данных.
    #[error("execution budget {budget:?} does not fit the sandbox window {window:?}: refused before launch")]
    Unfit { window: Duration, budget: Duration },
    /// Сокет лежит в записываемом дереве: bind рабочей копии/времянки перекрыл бы его,
    /// а агент успел бы подменить файл между проверкой и монтированием.
    #[error("socket `{path}` lies inside a writable directory: refused before launch")]
    SocketInside { path: PathBuf },
    /// Сокета нет: отсутствие канала — отдельное состояние, а не разрешение.
    #[error("socket `{path}` does not exist: refused before launch")]
    SocketMissing { path: PathBuf },
    /// Путь есть, но это не unix-сокет (симлинк тоже сюда): подмена дала бы чужой канал.
    #[error("`{path}` is not a unix socket: refused before launch")]
    SocketNotSocket { path: PathBuf },
    /// Метаданные сокета не читаются: «не посмотрели» — не «разрешено».
    #[error("socket `{path}` cannot be inspected: {source}")]
    SocketUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
        self.argv_with_socket(program, args, None)
    }

    /// Тот же вектор, плюс один `--ro-bind` на файл хостового unix-сокета. Сокет монтируется
    /// ДО записываемых каталогов и только на чтение: подменить его изнутри нельзя.
    fn argv_with_socket(
        &self,
        program: &Path,
        args: &[&str],
        socket: Option<&Path>,
    ) -> Vec<OsString> {
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
        if let Some(socket) = socket {
            Self::bind(&mut argv, "--ro-bind", socket);
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
        self.command_with_socket(write, program, args, budget, None)
    }

    /// Команда с каналом модели: файл unix-сокета монтируется внутрь как `--ro-bind`.
    /// Путь проверяется ДО запуска: сокет в записываемом дереве перекрывается bind'ом
    /// рабочей копии, а отсутствующий или подменённый файл — это отсутствие канала,
    /// а не разрешение. `--unshare-net` при этом не снимается.
    pub fn command_with_socket(
        &self,
        write: &Path,
        program: &Path,
        args: &[&str],
        budget: Duration,
        socket: Option<&Path>,
    ) -> Result<Command, SandboxError> {
        self.preflight(write, budget)?;
        if let Some(socket) = socket {
            self.check_socket(socket)?;
        }
        let mut command = Command::new("bwrap");
        command.args(self.argv_with_socket(program, args, socket));
        Ok(command)
    }

    /// Три РАЗНЫХ отказа вместо одного «не годится»: место, отсутствие и не-сокет чинятся
    /// по-разному, и «не посмотрели» не должно становиться «разрешено».
    fn check_socket(&self, socket: &Path) -> Result<(), SandboxError> {
        let path = normalize(socket);
        let (work, tmp) = (normalize(&self.work), normalize(&self.tmp));
        if path.starts_with(&work) || path.starts_with(&tmp) {
            return Err(SandboxError::SocketInside { path });
        }
        // `symlink_metadata`, а не `metadata`: ссылка на сокет — это подмена канала.
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_socket() => Ok(()),
            Ok(_) => Err(SandboxError::SocketNotSocket { path }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(SandboxError::SocketMissing { path })
            }
            Err(source) => Err(SandboxError::SocketUnreadable { path, source }),
        }
    }

    /// Все отказы — до запуска, и все три — про саму песочницу. Их порядок не важен: каждый
    /// отказ называет свой виновник, и ни один не значит «запуск не удался».
    fn preflight(&self, write: &Path, budget: Duration) -> Result<(), SandboxError> {
        // `exists()` идёт по ссылке: `.git` файлом (как в worktree) историю тоже несёт.
        let history = self.work.join(".git");
        if history.exists() {
            return Err(SandboxError::History { path: history });
        }
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
