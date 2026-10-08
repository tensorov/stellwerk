//! Спецификация M6/packet-01: чужой секрет интегратору НЕЧИТАЕМ, а не «не напечатан».
//!
//! Проверка отрицательная: успех = значение не получено. Чтобы «не смог прочитать» и «нечего
//! было читать» не слились, сперва идёт положительная контрольная проба: секрет заложен,
//! прочитан хостом, достигнут отдельным процессом, а его копия под корнем bind'а доказана
//! видимой агенту. Три попытки независимы и различаются МЕХАНИЗМОМ: политика при выдаче
//! инструмента, namespace ядра и политика путей в коде.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use stellwerk_task::m5::integrator_profile::{
    IntegratorProfile, ModelFacts, ProfileError, FOREIGN_CONFIGS,
};
use stellwerk_task::m5::sandbox_bwrap::{Sandbox, SandboxError};
use stellwerk_task::m5::secrets::Token;
use stellwerk_task::m5::session_identity::{CodemodeState, IdentityError, SessionDescription};

const BUDGET: Duration = Duration::from_secs(30);
const WINDOW: Duration = Duration::from_secs(60);
const LIMIT: Duration = Duration::from_secs(300);
const TOUCH: [&str; 2] = ["/usr/bin/touch", "/bin/touch"];
/// Искусственный секрет: значение, которое можно напечатать, рано или поздно напечатают.
const MARKER: &str = "SW6-ARTIFICIAL-KEY-4f19c7a2-not-a-real-credential";
/// Имя, под которым ключ известен системе: именно его не должно быть ни в чём.
const NAME: &str = "STELLWERK_MODEL_KEY";

/// Фикстура: рабочая копия и конфиг СНАРУЖИ неё — иначе отказ был бы свойством раскладки.
struct Fixture {
    work: PathBuf,
    tmp: PathBuf,
    secret: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    let (root, tag) = (
        std::env::temp_dir(),
        format!("sw-m601-{tag}-{}", std::process::id()),
    );
    let f = Fixture {
        work: root.join(&tag).join("work"),
        tmp: root.join(&tag).join("tmp"),
        secret: root.join(format!("{tag}-out")).join("proxy.env"),
    };
    for dir in [&f.work, &f.tmp, f.secret.parent().expect("каталог секрета")] {
        fs::create_dir_all(dir).expect("каталог фикстуры создан");
    }
    f
}

fn profile(f: &Fixture) -> IntegratorProfile {
    IntegratorProfile::minimal(&f.work).expect("кодом")
}

/// Песочница строится из КОРНЕЙ ПРОФИЛЯ: разошлись бы — проверялся бы не тот профиль.
fn sandbox(f: &Fixture, p: &IntegratorProfile) -> Sandbox {
    let mut read_only = Vec::new();
    for d in p.system_roots().iter().filter(|d| d.is_dir()) {
        read_only.push(d.clone());
    }
    Sandbox {
        work: f.work.clone(),
        tmp: f.tmp.clone(),
        read_only,
        window: WINDOW,
    }
}

/// `/usr/bin/…` и `/bin/…` — одно и то же; пустая подстановка означала бы «проверять нечем».
fn touch() -> PathBuf {
    let found = TOUCH.iter().map(PathBuf::from).find(|p| p.is_file());
    found.unwrap_or_default()
}

/// Настоящий запуск `bwrap`. Её отсутствие — падение теста, а не повод объявить границу
/// проверенной: «песочницы нет» и «песочница не пускает» — разные результаты.
fn run(sb: &Sandbox, f: &Fixture, args: &[&str]) -> Output {
    let touch = touch();
    assert!(touch.is_file(), "нет ни одного из {TOUCH:?}");
    let mut cmd = sb.command(&f.work, &touch, args, BUDGET).expect("запуск");
    cmd.output().expect("нет `bwrap` — нет и границы")
}

fn s(path: &Path) -> &str {
    path.to_str().expect("путь из temp_dir валиден")
}

/// ПОЛОЖИТЕЛЬНАЯ КОНТРОЛЬНАЯ ПРОБА: секрет есть, лежит там, куда дотягивается внешний
/// наблюдатель, и песочница рабочая. Без неё «агент не прочитал» и «читать было нечего» дают
/// один зелёный — ровно эта подмена портила packet-07, где `bwrap` сам создавал каталоги.
#[test]
fn the_planted_secret_really_is_there_and_reachable_from_outside() {
    let f = fixture("control");
    fs::write(&f.secret, MARKER).expect("секрет подложен");
    // 1. Файл есть, и это ровно те байты, которые искали: и длина, и содержимое.
    let len = fs::metadata(&f.secret).expect("файл секрета есть").len() as usize;
    assert_eq!(len, MARKER.len(), "длина секрета");
    assert_eq!(fs::read(&f.secret).expect("хост читает"), MARKER.as_bytes());
    // 2. Внешний наблюдатель — отдельный процесс, а не только сам тест.
    let mut probe = Command::new(touch());
    let plain = probe.arg(&f.secret).output().expect("контроль");
    assert!(plain.status.success(), "хост достаёт секрет: {plain:?}");
    // 3. Песочница не сломана: копия ПОД корнем bind'а видна агенту, значит отказ ниже — про
    //    путь, а не про `bwrap`, который и не смог бы ничего показать.
    let p = profile(&f);
    let inner = f.work.join("integrator.json");
    fs::write(&inner, MARKER).expect("копия под корнем записана");
    let visible = run(&sandbox(&f, &p), &f, &[s(&inner)]);
    assert!(visible.status.success(), "виден агенту: {visible:?}");
}

/// Путь 1 — ИНСТРУМЕНТ. Отказ при выдаче, до запуска чего-либо. Отказ печатается через
/// `#[error]` в любой отчёт, поэтому проверяется именно ТЕКСТ, а не только вариант.
#[test]
fn the_integrators_own_tool_call_is_refused_before_anything_runs() {
    let f = fixture("tool");
    fs::write(&f.secret, MARKER).expect("секрет подложен");
    let p = profile(&f);
    let src = CodemodeState::AbsentAfterConfigLoad;
    let session = SessionDescription::new("integrator-1", p.work(), p.tools().clone(), LIMIT, src)
        .expect("описание сессии");
    let asked = session.adjudicate("config_read", None);
    let refused = asked.expect_err("нет инструмента чтения конфига");
    assert!(
        matches!(refused, IdentityError::NoSuchTool { .. }),
        "{refused:?}"
    );
    let shown = format!("{refused}{refused:?}");
    assert!(!shown.contains(MARKER), "отказ без секрета: {shown}");
    assert!(!shown.to_lowercase().contains("authorization"), "эха нет");
}

/// Путь 2 — SHELL. Настоящий `bwrap` из корней профиля: у процесса в namespace нет ни пути,
/// ни его каталога. Тот же путь отвергается и ДО запуска — отказ не зависит от процесса.
#[test]
fn the_kernel_denies_the_secret_path_to_the_integrators_shell() {
    let f = fixture("shell");
    fs::write(&f.secret, MARKER).expect("секрет подложен");
    let sb = sandbox(&f, &profile(&f));
    let denied = run(&sb, &f, &[s(&f.secret)]);
    assert!(
        !denied.status.success(),
        "секрет не в песочнице: {denied:?}"
    );
    // Файл на хосте на месте: отказ ядра — «пути нет», а не «файла нет», и эти два случая
    // требовалось различить. Раньше packet-07 на них подменял друг друга.
    let len = fs::metadata(&f.secret).expect("файл на месте").len() as usize;
    assert_eq!(len, MARKER.len(), "отказ не вызван исчезновением файла");
    let target = sb.command(&f.secret, &touch(), &[], BUDGET);
    let refused = target.expect_err("вне копии");
    assert!(
        matches!(refused, SandboxError::Outside { .. }),
        "{refused:?}"
    );
    assert!(!refused.to_string().contains(MARKER), "отказ без секрета");
}

/// Путь 3 — ЧТЕНИЕ КОНФИГА. Единственная дверь агента — `admits`. Тут же перечень конфигов и
/// сведения о модели `{id, limits, caps}`.
#[test]
fn the_profile_refuses_the_foreign_config_and_model_facts_carry_no_credential_field() {
    let f = fixture("cfg");
    fs::write(&f.secret, MARKER).expect("секрет подложен");
    let p = profile(&f);
    let refused = p.admits(&f.secret).expect_err("секрет вне профиля");
    assert_eq!(
        refused,
        ProfileError::OutsideProfile {
            roots: p.roots(),
            path: f.secret.clone()
        }
    );
    // Ни один конфиг из перечня в коде не должен оказаться под корнем профиля.
    for config in FOREIGN_CONFIGS {
        assert!(p.admits(Path::new(config)).is_err(), "виден: {config}");
    }
    // Положительная ветка `admits` проверена тоже: иначе «профиль ничего не открывает» и
    // «профиль открывает ровно своё» дали бы один и тот же результат.
    let own = f.work.join("task.md");
    let sys = Path::new("/usr/lib");
    assert!(p.admits(&own).is_ok(), "рабочий каталог виден: {own:?}");
    assert!(p.admits(sys).is_ok(), "системный корень виден: {sys:?}");
    // `..` границу не обходит: путь нормализован лексически, до того как дойдёт до ФС.
    let escape = f.work.join("..").join("out").join("proxy.env");
    assert!(p.admits(&escape).is_err(), "обход через `..`: {escape:?}");
    let facts = ModelFacts::parse("id=llm-7b;limits=ctx=32k,rpm=60;caps=chat,embed")
        .expect("каталог без ключа разобран");
    assert_eq!(facts.id, "llm-7b");
    assert_eq!(facts.limits, ["ctx=32k", "rpm=60"]);
    assert_eq!(facts.caps, ["chat", "embed"]);
    // Пустая ячейка — не пустое имя: `caps=chat,,embed` это ДВЕ возможности, а не три.
    let padded = ModelFacts::parse("id=llm-7b;caps=chat,,embed").expect("разобрано");
    assert_eq!(padded.caps, ["chat", "embed"], "пустая ячейка не имя");
    // Перечень полей ЗАКРЫТ: строка с ключом не «разбирается частично», а отказывает целиком.
    let line = format!("id=llm-7b;api_key={MARKER}");
    let carrying = ModelFacts::parse(&line).expect_err("поле вне перечня");
    assert_eq!(
        carrying,
        ProfileError::UnlistedField {
            field: "api_key".to_owned()
        }
    );
    assert!(!carrying.to_string().contains(MARKER), "нет значения");
    // Отсутствие данных — свой вариант, а не `id=""`: «не задано» и «задано в пустоту» (закон 6).
    let no_id = ModelFacts::parse("caps=chat").expect_err("без `id`");
    assert_eq!(no_id, ProfileError::AbsentField { field: "id" });
    let twice = ModelFacts::parse("id=a;id=b").expect_err("повтор");
    assert_eq!(twice, ProfileError::RepeatedField { field: "id" });
    assert_eq!(
        ModelFacts::parse("id").expect_err("без `=`"),
        ProfileError::Malformed
    );
}

/// Секрет не доходит ни до `Debug`, ни до текста отказа, ни до строки журнала. Проверяется и
/// сам модуль: в нём нет ни чтения файла, ни чтения окружения, ни `Display`.
#[test]
fn no_secret_material_reaches_debug_an_error_a_log_line_or_the_module_source() {
    let f = fixture("leak");
    fs::write(&f.secret, MARKER).expect("секрет подложен");
    let p = profile(&f);
    let value = fs::read_to_string(&f.secret).expect("маркер прочитан хостом");
    let token = Token::new(NAME, &value).expect("имя и значения заданы");
    // `Token` напечатал бы длину, `#[error]` — виновника: оба канала проверяются разом.
    let refused = p.admits(&f.secret).expect_err("секрет вне профиля");
    let shown = format!("{token:?}|{refused}|{:?}|{refused:?}", p.tools());
    assert!(!shown.contains(MARKER), "вывод без значения: {shown}");
    let log = format!(
        "{:?}",
        ModelFacts::parse("id=llm-7b;caps=chat").expect("каталог")
    );
    assert!(!log.contains(MARKER), "модель без значения: {log}");
    let source = include_str!("../src/m5/integrator_profile.rs");
    for banned in [
        "fs::read",
        "read_to_string",
        "std::env",
        "impl Display",
        "impl fmt::Display",
    ] {
        assert!(!source.contains(banned), "канал вывода: `{banned}`");
    }
}
