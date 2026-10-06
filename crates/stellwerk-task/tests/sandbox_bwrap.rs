//! Спецификация M5/packet-07a: вектор аргументов `bwrap` и конфигурация от вызывающего.
//!
//! Проверяется то, что обязано быть выражено в векторе до запуска чего бы то ни было: сеть
//! выключена ровно одним флагом, системные каталоги приходят только на чтение, на запись —
//! ровно два своих каталога, и у каждого bind'а `SRC` совпадает с `DEST`. Пропуск адресата
//! не диагностируется — `bwrap` принимает следующий флаг за адресат и падает позже, уже
//! внутри песочницы, — поэтому инвариант проверяется здесь явно, а не «по числу bind'ов».

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_task::m5::sandbox_bwrap::Sandbox;

/// Окно песочницы — поле конфигурации; отказ по нему приходит в 07b.
const WINDOW: Duration = Duration::from_secs(60);

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
