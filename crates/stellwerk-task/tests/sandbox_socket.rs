//! Спецификация N2-00c: канал модели — файл unix-сокета, отданный внутрь ТОЛЬКО на чтение.
//!
//! Сеть не открывается: `--unshare-net` остаётся на месте. Сокет в записываемом дереве
//! отвергается ДО запуска — иначе bind рабочей копии перекрыл бы его, а агент успел бы
//! подменить файл между проверкой и монтированием. Отсутствие сокета — отдельный отказ,
//! а не молчаливое «канал разрешён».

use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use stellwerk_task::m5::sandbox_bwrap::{Sandbox, SandboxError};

/// Окно песочницы больше бюджета: «окно кончилось» и «бюджет кончился» — разные отказы.
const WINDOW: Duration = Duration::from_secs(60);
/// Бюджет одного запуска.
const BUDGET: Duration = Duration::from_secs(30);
/// Писатель: отказ приходит до запуска, и маркер это доказывает.
const TOUCH: [&str; 2] = ["/usr/bin/touch", "/bin/touch"];

/// Системные каталоги только на чтение. `/lib` — для интерпретатора, `/usr` — для `python3`.
fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .iter()
        .map(|p| PathBuf::from(*p))
        .filter(|p| p.is_dir())
        .collect()
}

/// Программа из системных каталогов: путь вне песочницы не запустится вовсе.
fn in_system(names: &[&str]) -> PathBuf {
    let mut candidates = names.iter().map(|p| PathBuf::from(*p));
    let found = candidates.find(|p| p.is_file());
    found.unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

fn str_of(path: &Path) -> &str {
    path.to_str().expect("путь из temp_dir валиден")
}

fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-n2c-{tag}-{}", std::process::id()))
}

fn sandbox_at(root: &Path) -> Sandbox {
    Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    }
}

/// Каталоги фикстуры: рабочая копия, времянка и **хозяйский** каталог сокета. Сокет в
/// `work`/`tmp` был бы записываемым, поэтому хозяйский каталог лежит рядом и отдельно.
fn fixture(tag: &str) -> (PathBuf, Sandbox, PathBuf) {
    let root = fixture_root(tag);
    let (work, tmp, host) = (root.join("work"), root.join("tmp"), root.join("host"));
    for dir in [&work, &tmp, &host] {
        fs::create_dir_all(dir).expect("каталог фикстуры создан");
    }
    (root.clone(), sandbox_at(&root), host)
}

/// п. 1: сокет — один `--ro-bind` на ФАЙЛ, сети нет, записываемых каталогов по-прежнему два.
#[test]
fn socket_seam_keeps_unshare_net_and_binds_read_only() {
    let (_root, sandbox, host) = fixture("argv");
    let socket = host.join("m.sock");
    let _listener = UnixListener::bind(&socket).expect("хост слушает сокет");

    let cmd = sandbox
        .command_with_socket(
            &sandbox.work,
            &in_system(&["/usr/bin/true", "/bin/true"]),
            &[],
            BUDGET,
            Some(&socket),
        )
        .expect("сокет вне записываемого дерева — запуск разрешён");
    let argv: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let dump = format!("{argv:?}");
    let sock = str_of(&socket);

    // Сети нет только здесь; дальше «нет сети» выразить нечем.
    assert!(argv.contains(&"--unshare-net".to_owned()), "{dump}");
    // Сокет отдан ровно одним ro-bind'ом, источник и адресат совпадают.
    assert!(
        argv.windows(3)
            .any(|w| w[0] == "--ro-bind" && w[1] == sock && w[2] == sock),
        "{dump}"
    );
    // Записываемым он не стал: `--bind` на сокет не появился.
    assert!(
        !argv.windows(3).any(|w| w[0] == "--bind" && w[1] == sock),
        "{dump}"
    ); // Канал не расширяет запись: по-прежнему ровно два записываемых каталога.
    assert_eq!(argv.iter().filter(|a| *a == "--bind").count(), 2, "{dump}");
}

/// п. 2: сокет в `work`/`tmp` перекрывается bind'ом дерева и подменяем агентом — отказ до запуска.
#[test]
fn a_socket_inside_the_writable_tree_is_refused_before_launch() {
    let (_root, sandbox, _host) = fixture("inside");
    for dir in [sandbox.work.clone(), sandbox.tmp.clone()] {
        let inside = dir.join("m.sock");
        // Настоящий сокет: отказ обязан быть про МЕСТО, а не про отсутствие файла.
        let _listener = UnixListener::bind(&inside).expect("сокет создан");
        let marker = sandbox.work.join("marker");
        let args = [str_of(&marker)];
        let refused = sandbox.command_with_socket(
            &sandbox.work,
            &in_system(&TOUCH),
            &args,
            BUDGET,
            Some(&inside),
        );
        match refused {
            Err(SandboxError::SocketInside { path }) => assert_eq!(path, inside),
            other => panic!("сокет в записываемом дереве — отказ, а не {other:?}"),
        }
        // Маркер создал бы процесс, которого не было: отказ пришёл ДО `spawn`.
        assert!(!marker.exists());
    }
}

/// п. 3 (закон 6): «не прочитали» — не «сокета нет». Родитель-файл даёт `ENOTDIR`
/// независимо от прав пользователя, поэтому различие проверяется на любой машине.
#[test]
fn an_unreadable_socket_path_is_not_reported_as_absent() {
    let (root, sandbox, _host) = fixture("unreadable");
    let blocker = root.join("host").join("blocker");
    fs::write(&blocker, b"not a directory").expect("файл-блокиратор записан");
    let path = blocker.join("m.sock");
    let refused =
        sandbox.command_with_socket(&sandbox.work, &in_system(&TOUCH), &[], BUDGET, Some(&path));
    match refused {
        Err(SandboxError::SocketUnreadable {
            path: refused_path, ..
        }) => assert_eq!(refused_path, path),
        other => panic!("нечитаемый путь — `SocketUnreadable`, а не {other:?}"),
    }
}

/// п. 3 (закон 6): «сокета нет» и «это не сокет» — разные состояния, и ни одно не `Ok`.
#[test]
fn an_absent_socket_is_absent_not_allowed() {
    let (root, sandbox, _host) = fixture("absent");
    let missing = root.join("host").join("nope.sock");
    let refused = sandbox.command_with_socket(
        &sandbox.work,
        &in_system(&TOUCH),
        &[],
        BUDGET,
        Some(&missing),
    );
    match refused {
        Err(SandboxError::SocketMissing { path }) => assert_eq!(path, missing),
        other => panic!("отсутствующий сокет — `SocketMissing`, а не {other:?}"),
    }

    // Симлинк на настоящий сокет тоже отказ: `symlink_metadata` видит ссылку, а не сокет.
    let real = root.join("host").join("real.sock");
    let _listener = UnixListener::bind(&real).expect("сокет создан");
    let link = root.join("host").join("link.sock");
    std::os::unix::fs::symlink(&real, &link).expect("ссылка создана");
    let refused =
        sandbox.command_with_socket(&sandbox.work, &in_system(&TOUCH), &[], BUDGET, Some(&link));
    match refused {
        Err(SandboxError::SocketNotSocket { path }) => assert_eq!(path, link),
        other => panic!("ссылка на сокет — `SocketNotSocket`, а не {other:?}"),
    }

    let plain = root.join("host").join("plain.txt");
    fs::write(&plain, b"not a socket").expect("файл записан");
    let refused =
        sandbox.command_with_socket(&sandbox.work, &in_system(&TOUCH), &[], BUDGET, Some(&plain));
    match refused {
        Err(SandboxError::SocketNotSocket { path }) => assert_eq!(path, plain),
        other => panic!("обычный файл — `SocketNotSocket`, а не {other:?}"),
    }
}

/// п. 4: канал работает по-настоящему и без сети. Клиент внутри `bwrap` шлёт `PING`,
/// хост отвечает `PONG`. Если бы сокет не был смонтирован, клиент не подключился бы.
#[test]
fn a_sandboxed_client_reaches_the_host_socket_without_network() {
    let (_root, sandbox, host) = fixture("roundtrip");
    let socket = host.join("m.sock");
    let listener = UnixListener::bind(&socket).expect("хост слушает сокет");
    listener
        .set_nonblocking(true)
        .expect("неблокирующий accept для дедлайна");

    let script = "import socket,sys\n\
                  s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)\n\
                  s.connect(sys.argv[1])\n\
                  s.sendall(b'PING')\n\
                  print(s.recv(4).decode())\n";
    let mut cmd = sandbox
        .command_with_socket(
            &sandbox.work,
            &in_system(&["/usr/bin/python3", "/bin/python3"]),
            &["-c", script, str_of(&socket)],
            BUDGET,
            Some(&socket),
        )
        .expect("сокет вне записываемого дерева");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn().expect("клиент в песочнице запущен");

    let mut stream = accept_by(&listener, Duration::from_secs(20));
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("дедлайн на чтение");
    let mut ping = [0u8; 4];
    stream.read_exact(&mut ping).expect("клиент шлёт PING");
    assert_eq!(&ping, b"PING");
    stream.write_all(b"PONG").expect("ответ записан");

    let out = child.wait_with_output().expect("клиент завершился");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("PONG"), "stdout={stdout} stderr={stderr}");
}

/// Дедлайн вместо вечного `accept`: зависший тест не отличим от «канал не работает».
fn accept_by(listener: &UnixListener, limit: Duration) -> UnixStream {
    let deadline = Instant::now() + limit;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "клиент в песочнице не подключился за {limit:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept: {e}"),
        }
    }
}
