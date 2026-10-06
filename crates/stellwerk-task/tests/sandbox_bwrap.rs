//! Спецификация M5/packet-07a–07c: песочница `bwrap` и её границы.
//!
//! Проверяет не код, а ядро: команда внутри пишет, команда с внешним путём получает отказ
//! ОТ ЯДРА, а путь для записи наружу отвергается ДО запуска — и о том, что процесса не
//! было, говорит отсутствие маркера, который он создал бы. Отсутствие `bwrap` — падение,
//! а не пропуск: нет движка, значит границы не проверены.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use stellwerk_task::m5::sandbox_bwrap::{Sandbox, SandboxError};

/// Окно песочницы больше бюджета: иначе «окно кончилось» и «бюджет кончился» были бы
/// одним и тем же словом про одно и то же.
const WINDOW: Duration = Duration::from_secs(60);
/// Бюджет одного запуска: столько ждём конкретную команду.
const BUDGET: Duration = Duration::from_secs(30);
/// Писатель и его системные алиасы: путь вне песочницы не запустится вовсе.
const TOUCH: [&str; 2] = ["/usr/bin/touch", "/bin/touch"];

/// Системные каталоги — только на чтение. Список ДАН, а не угадан: `--ro-bind` на
/// отсутствующий источник `bwrap` считает ошибкой, а какие каталоги есть — свойство
/// машины, а не контракт песочницы.
fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib64", "/etc"]
        .iter()
        .map(|p| PathBuf::from(*p))
        .filter(|p| p.is_dir())
        .collect()
}

/// Программа берётся из системных каталогов: `/usr/bin/…` и `/bin/…` — одно и то же.
/// Путь вне песочницы не запустится вовсе, а молчаливый `None` означал бы «проверять нечем».
fn in_system(names: &[&str]) -> PathBuf {
    let mut candidates = names.iter().map(|p| PathBuf::from(*p));
    let found = candidates.find(|p| p.is_file());
    found.unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

/// Путь из `Path` в `&str`: фикстуры лежат в `temp_dir`, имя им даёт процесс.
fn str_of(path: &Path) -> &str {
    path.to_str().expect("путь из temp_dir валиден")
}

/// OsString в строку: вектор читается глазами, а не через `Debug` с кавычками.
fn as_text(arg: &std::ffi::OsString) -> String {
    arg.to_string_lossy().into_owned()
}

/// Каталог фикстуры под текущим процессом: имя по тегу, чтобы прогоны разных тестов не
/// делили одну рабочую копию.
fn fixture_root(tag: &str) -> PathBuf {
    let named = format!("sw-p07-{tag}-{}", std::process::id());
    std::env::temp_dir().join(named)
}

/// Песочница собирается из корня фикстуры: системные каталоги на чтение, свои — на запись.
/// Каталоги создаёт вызывающий, а не конструктор: собирать список песочниц нельзя, создавая
/// каталоги, иначе «узнать, кто жив» стоило бы записи на диск.
fn sandbox_at(root: &Path) -> Sandbox {
    Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    }
}

/// п. 1 и forbidden: вектор — список, с `--unshare-net`, с каждым bind'ом по тому же пути,
/// ровно с двумя записываемыми каталогами, и без аргументов, которые выходят из песочницы.
#[test]
fn the_argument_vector_has_no_network_and_writes_only_its_own_directories() {
    let root = fixture_root("argv");
    let sandbox = sandbox_at(&root);
    let program = in_system(&["/usr/bin/true", "/bin/true"]);
    let built = sandbox.argv(&program, &["arg"]);
    let argv: Vec<String> = built.iter().map(as_text).collect();
    let dump = format!("{argv:?}");

    // Сети нет только здесь; дальше «нет сети» выразить нечем.
    assert!(argv.contains(&"--unshare-net".to_owned()), "{dump}");
    let count_of = |flag: &str| argv.iter().filter(|a| *a == flag).count();
    assert_eq!(count_of("--ro-bind"), system_dirs().len(), "{dump}");
    assert_eq!(count_of("--bind"), 2, "{dump}");
    // SRC == DEST у каждого bind'а: без адресата `bwrap` съедает следующий флаг.
    for at in 0..argv.len().saturating_sub(2) {
        let flag = argv[at].as_str();
        if flag == "--bind" || flag == "--ro-bind" {
            assert_eq!(argv[at + 1], argv[at + 2], "{dump}");
        }
    }
    assert!(argv.iter().all(|a| !a.contains("..")), "{dump}");
    // Пишутся только свои каталоги: bind наружу отдал бы кусок хостовой ФС.
    for target in [str_of(&sandbox.work), str_of(&sandbox.tmp)] {
        let mut writable = argv.windows(3).filter(|w| w[0] == "--bind");
        assert!(writable.any(|w| w[1] == target), "{target}: {dump}");
        assert!(Path::new(target).starts_with(&root), "{dump}");
    }
}

/// п. 2 и п. 6: отказ ДО запуска. `nested/` на хосте не существует и не мешает — проверка
/// лексическая, а не «по существованию»; виновник назван ПОСЛЕ нормализации `..`.
#[test]
fn a_write_path_that_escapes_after_dot_dot_is_refused_before_the_process_starts() {
    let root = fixture_root("preflight");
    let sandbox = sandbox_at(&root);
    let work = sandbox.work.clone();
    let touch = in_system(&TOUCH);
    let escapes = [
        work.join("../outside.txt"),
        work.join("nested/../../outside.txt"),
    ];
    for escape in &escapes {
        let marker = work.join("marker");
        let args = [str_of(&marker)];
        match sandbox.command(escape, &touch, &args, BUDGET) {
            // Назван нормализованный путь, а не подставленный на вход.
            Err(SandboxError::Outside { path, .. }) => {
                assert_eq!(path, root.join("outside.txt"))
            }
            other => panic!("путь снаружи — отказ, а не {other:?}"),
        }
        // Маркер создал бы процесс, которого не было: отказ пришёл ДО `spawn`.
        assert!(!marker.exists());
        assert!(!root.join("outside.txt").exists());
    }
    // `..` дальше корня песочницы: подниматься больше некуда, и выход всё равно запрещён.
    let far = work.join("..").join("..").join("stray");
    let refused = sandbox.command(&far, &touch, &[], BUDGET);
    assert!(matches!(refused, Err(SandboxError::Outside { .. })));
}

/// п. 4: окно песочницы и бюджет исполнения — разные часы. Окно кончилось — следующий запуск
/// не начнётся и терять нечего; бюджет кончился — группа убита и частичные данные потеряны
/// (это `EngineError::TimedOut`, и песочница его не подменяет).
#[test]
fn an_execution_budget_that_does_not_fit_the_window_is_refused_and_loses_nothing() {
    let root = fixture_root("window");
    let tight = Sandbox {
        window: Duration::ZERO,
        ..sandbox_at(&root)
    };
    let marker = tight.work.join("marker");
    let args = [str_of(&marker)];
    let refused = tight.command(&tight.work, &in_system(&TOUCH), &args, BUDGET);
    match refused {
        // Названы и окно, и бюджет: «не помещается» не спутать с «процесс умер».
        Err(SandboxError::Unfit { window, budget }) => {
            assert_eq!((window, budget), (Duration::ZERO, BUDGET))
        }
        other => panic!("бюджет не в окне — отказ, а не {other:?}"),
    }
    assert!(!marker.exists());
}

/// Настоящий запуск: `bwrap` обязан быть в системе, и его отсутствие — падение теста, а не
/// повод объявить границы проверенными.
fn launch(sandbox: &Sandbox, write: &Path, args: &[&str]) -> Output {
    let cmd = sandbox.command(write, &in_system(&TOUCH), args, BUDGET);
    let mut cmd = cmd.expect("внутри рабочей копии запуск разрешён");
    cmd.output()
        .expect("нет движка — нет и проверенной границы")
}

/// Песочница и её фикстуры рядом, но порознь: рабочая копия, каталог временных файлов и
/// ВНЕШНЯЯ цель, лежащая не под их корнем. Под корнем она была бы создана как родитель
/// bind'а, и её недостижимость доказывала бы ровно ничего.
struct Fixture {
    root: PathBuf,
    work: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = fixture_root(tag);
        let out_dir = fixture_root(&format!("{tag}-outside"));
        let (work, tmp) = (root.join("work"), root.join("tmp"));
        fs::create_dir_all(&work).expect("рабочая копия создана");
        fs::create_dir_all(&tmp).expect("каталог временных файлов создан");
        fs::create_dir_all(&out_dir).expect("каталог внешней фикстуры создан");
        fs::write(work.join("state.txt"), "выгрузка").expect("выгрузка записана");
        Self {
            root,
            work,
            outside: out_dir.join("fixture.txt"),
        }
    }

    fn sandbox(&self) -> Sandbox {
        sandbox_at(&self.root)
    }
}

/// п. 1: команда внутри рабочей копии пишет, и написанное читается после прогона.
#[test]
fn a_command_inside_the_working_copy_writes_what_it_was_allowed_to_write() {
    let f = Fixture::new("inside");
    let out = launch(&f.sandbox(), &f.work, &["written.txt"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(f.work.join("written.txt").is_file());
}

/// п. 6: изоляцию доказывает ядро, а не наш валидатор. Контроль БЕЗ песочницы обязан
/// сработать — иначе «отказ» могла бы дать сама фикстура; в песочнице тот же вызов обязан
/// провалиться, и на хосте ничего не появиться.
#[test]
fn a_sandboxed_process_is_denied_by_the_kernel_where_a_plain_process_is_not() {
    let f = Fixture::new("kernel");
    let touch = in_system(&TOUCH);
    let plain = Command::new(&touch).arg(&f.outside).output();
    let plain = plain.expect("контроль запускается");
    // Без песочницы внешний путь доступен — иначе «отказ» дала бы сама фикстура.
    assert!(plain.status.success(), "{plain:?}");
    let gone = fs::remove_file(&f.outside);
    gone.expect("фикстура убрана, иначе отказ не отличить от «уже есть»");

    let args = [str_of(&f.outside)];
    let cmd = f.sandbox().command(&f.work, &touch, &args, BUDGET);
    let out = cmd.expect("запуск разрешён").output().expect("`bwrap`");
    // Отказ ОТ ЯДРА: пути нет в namespace, а не «наш валидатор сказал нет».
    assert!(!out.status.success(), "{out:?}");
    assert!(!f.outside.exists());
}

/// п. 3: внутри нет ни истории, ни файлов репозитория, а копия с `.git` отвергается —
/// «пустой каталог `.git`» и «нет `.git`» суть разные вещи.
#[test]
fn a_working_copy_holds_neither_history_nor_repository_files() {
    let f = Fixture::new("history");
    let out = launch(&f.sandbox(), &f.work, &["written.txt"]);
    assert!(out.status.success());

    let dir = fs::read_dir(&f.work).expect("рабочая копия читается");
    let mut names: Vec<String> = dir
        .map(|e| {
            e.expect("запись читается")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["state.txt", "written.txt"]);
    assert!(!f.work.join(".git").exists());
    assert!(!f.work.join("Cargo.toml").exists());

    let cloner = Fixture::new("cloner");
    fs::create_dir_all(cloner.work.join(".git")).expect("клон создан");
    let cloned = cloner
        .sandbox()
        .command(&cloner.work, &in_system(&TOUCH), &[], BUDGET);
    match cloned {
        // Виновник назван: внутри `.git` — это клон, а не выгрузка состояния.
        Err(SandboxError::History { path }) => assert!(path.ends_with(".git"), "{path:?}"),
        other => panic!("`.git` внутри — клон, а не {other:?}"),
    }
}
