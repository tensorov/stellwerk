//! Спецификация M5/packet-08: идентичность сессии и разрешённые инструменты.
//!
//! Проверяет не состав описания, а отказ: субагент не получает ничего сверх родителя,
//! обрезка набора — ошибка, а не укороченный список, а «codemode выключен» считается
//! доказанным только после загрузки конфигурации. Фикстура ничего не создаёт и не запускает,
//! поэтому отказ здесь не может выдать сама фикстура — как и в `sandbox_bwrap.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_task::m5::session_identity::{
    CallSource, CodemodeState, IdentityError, SessionDescription, ToolSet,
};

/// Лимит времени в описании — данные описания, а не бюджет исполнения: его считает
/// вызывающий (`m5::EngineAdapter::session`), и подмена одного другим сделала бы два разных
/// инцидента одним.
const LIMIT: Duration = Duration::from_secs(30);

/// Имя codemode как его знает Pi: `docs/PI_1_0_2_PROBE.md`, пункт (a) — встроенным он не
/// является, значит и «включён по умолчанию» быть не может.
const CODEMODE: &str = "codemode";

/// Набор для проверок, из которого убрать нечего: список, а не запрет.
fn listed(tools: &[&str]) -> ToolSet {
    ToolSet::listed(tools).expect("явный список без codemode")
}

/// Набор в одну строку: порядок сохраняется, а `Debug` с кавычками не читается.
fn names_of(tools: &ToolSet) -> String {
    tools.names().join(",")
}

/// Рабочий каталог фикстуры. Каталогов не создаём: граница проверяется лексически, и
/// существование каталога к делу не относится — за него отвечает запуск.
fn work_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-p08-{tag}"))
}

/// Сессия с проверенным «codemode выключен ПОСЛЕ загрузки конфигурации»: иначе отказ был бы
/// не про инструменты, а про непроверенное выключение (п. 8).
fn session_in(work: &Path, tools: &ToolSet) -> SessionDescription {
    let off = CodemodeState::AbsentAfterConfigLoad;
    SessionDescription::new("m5-p08", work, tools.clone(), LIMIT, off).expect("описание собрано")
}

/// П. 1, п. 2 и п. 5: движку уходит РОВНО рабочий каталог и перечисленные инструменты,
/// идентификатор попадает и в журнал, и в отчёт. Контроль обязателен: каталог сессии не
/// совпадает с каталогом процесса, поэтому равенство доказывает, что значение пришло из
/// описания, а не подтянулось из окружения — и секрету в окружении нет места.
#[test]
fn the_engine_is_given_exactly_the_working_directory_and_the_listed_tools() {
    let work = work_root("engine");
    let tools = listed(&["read", "grep"]);
    let session = session_in(&work, &tools);

    assert_ne!(
        work,
        std::env::current_dir().expect("каталог процесса"),
        "каталог сессии обязан отличаться от каталога процесса"
    );
    assert_eq!(session.cwd(), work.as_path());
    assert_eq!(session.limit(), LIMIT);
    // Явный список: инструмента нет в списке — он недоступен, а не «разрешён, если не
    // запрещён». И пустой список не значит «разрешено всё, чего не запрещено».
    assert_eq!(names_of(session.tools()), "read,grep");
    assert!(!session.tools().holds("write"));
    assert!(!session.tools().holds(CODEMODE));
    let empty = listed(&[]);
    assert_eq!(names_of(&empty), "");
    assert!(!empty.holds("read"));
    // П. 5: по идентификатору восстанавливается, кто с какими правами работал. Журнал и
    // отчёт — разные документы, и идентификатор обязан быть в обоих. Сверка ИДЕНТИФИФИКАТОРА
    // КОНСТАНТОЙ, а не вызовом `id()`: `contains(id())` сравнивал бы текст с ним же, и
    // «`id()` вернул пустую строку» прошло бы незаметно — это поймал `cargo mutants`.
    let (journal, report) = (session.journal_line("start"), session.report());
    for text in [&journal, &report] {
        assert!(text.contains("m5-p08"), "{text}");
    }
    assert_eq!(session.id(), "m5-p08");
    assert!(report.contains(&names_of(&tools)), "{report}");
    assert!(report.contains("30s"), "лимит времени — в отчёте: {report}");
}

/// П. 3: запрос шире родителя отклоняется ЦЕЛИКОМ. Частичной выдачи нет — и это не вопрос
/// вежливости: обрезанный по родителю набор и есть утечка остатка.
#[test]
fn a_subagent_asking_beyond_its_parent_is_refused_whole_and_leaks_no_remainder() {
    let parent = listed(&["read", "grep"]);
    // Контроль: ровно те инструменты, что есть у родителя, — разрешение, иначе отказ ниже
    // мог бы дать сама фикстура, отказавшая бы всему подряд.
    let exact = stellwerk_task::m5::session_identity::grant(&parent, &["read"]);
    assert_eq!(names_of(&exact.expect("родительское — разрешено")), "read");

    let asked = ["read", "write", "net"];
    match stellwerk_task::m5::session_identity::grant(&parent, &asked) {
        Err(IdentityError::Escalation { requested, absent }) => {
            assert_eq!(requested, ["read", "write", "net"]);
            // Виновник назван целиком: «не хватило write и net», а не «не хватило всего».
            assert_eq!(absent, ["write", "net"]);
        }
        // Укороченный список здесь был бы УСПЕХОМ с другими правами — и это не отказ.
        other => panic!("запрос шире родителя — отказ, а не {other:?}"),
    }
}

/// П. 4: обещанное число инструментов и переданный список обязаны совпасть. Расхождение —
/// отказ, а не тихая обрезка: обрезанный набор выглядит как успешный запуск с другими
/// правами, и этого диагностировать нельзя.
#[test]
fn a_truncated_tool_list_is_an_error_and_never_a_shorter_one() {
    // Обещано три, передано один: список потерял хвост по дороге.
    match ToolSet::exactly(3, &["read"]) {
        Err(IdentityError::Truncated { promised, given }) => {
            assert_eq!((promised, given), (3, 1));
        }
        other => panic!("усечение — ошибка, а не {other:?}"),
    }
    // Повтор тоже усечение: два места под один инструмент — это один инструмент, и обещание
    // о двух не выполнено. Тихая дедупликация выдала бы набор с правами, которых не просили.
    match ToolSet::exactly(2, &["read", "read"]) {
        Err(IdentityError::Truncated { promised, given }) => {
            assert_eq!((promised, given), (2, 1));
        }
        other => panic!("повтор — тоже усечение, а не {other:?}"),
    }
    // Совпадение проходит — иначе «усечение запрещено» читалось бы как «список запрещён».
    let exact = ToolSet::exactly(2, &["read", "grep"]).expect("обещание выполнено");
    assert_eq!(names_of(&exact), "read,grep");
}

/// П. 6 и п. 9: вложенный вызов помечен `parentToolCallId` (проба (a): те же хуки
/// `tool_call`, то же поле), опознаётся и проходит проверку ЗАНОВО, с исходными правами.
/// Происхождение не даёт ничего: то же решение, что и у прямого вызова.
#[test]
fn a_nested_call_is_checked_afresh_while_an_unrecognised_source_is_refused() {
    let session = session_in(&work_root("nested"), &listed(&["read"]));

    let direct = session.adjudicate("read", None).expect("прямой вызов");
    let nested = session
        .adjudicate("read", Some("toolu_parent"))
        .expect("вложенный вызов");
    // Права те же, источник виден, и ни одно поле решения не пришло от родителя.
    assert_eq!(
        (direct.session, &direct.tool),
        (nested.session, &nested.tool)
    );
    assert_eq!(direct.source, CallSource::Direct);
    assert_eq!(
        nested.source,
        CallSource::Nested {
            parent_tool_call_id: "toolu_parent".to_owned(),
        }
    );
    // Отсутствие инструмента у родителя отменяет и вложенный вызов: знание происхождения
    // ничего не наследует, поэтому проверка вложенного ЗАНОВО отказывает.
    for source in [None, Some("toolu_parent")] {
        let refused = session.adjudicate("write", source);
        assert!(
            matches!(refused, Err(IdentityError::NoSuchTool { .. })),
            "{source:?}"
        );
    }
    // Пустой идентификатор происхождения — это НЕ прямой вызов: источник не опознан, и
    // молчаливое «ну значит без родителя» выдало бы отказ за разрешение.
    match session.adjudicate("read", Some("   ")) {
        Err(IdentityError::UnrecognisedSource {
            parent_tool_call_id,
        }) => {
            assert_eq!(parent_tool_call_id, "   ");
        }
        other => panic!("неопознанный источник — отказ, а не {other:?}"),
    }
}

/// П. 7 и п. 8: codemode в рабочих и операторских сессиях не включается — он не входит в
/// набор, а «выключено по флагу запуска» не считается доказательством, потому что codemode
/// подключается сам через MCP. Отсюда три состояния, а не флаг: непроверенное выключение
/// равно непроверенному включению.
#[test]
fn codemode_is_not_in_the_set_and_its_absence_counts_only_after_the_config_load() {
    // В наборе его быть не может вовсе — даже если перечисление его попросило.
    match ToolSet::listed(&[CODEMODE, "read"]) {
        Err(IdentityError::CodemodeInSet { tool }) => assert_eq!(tool, CODEMODE),
        other => panic!("codemode в наборе — отказ, а не {other:?}"),
    }
    let work = work_root("codemode");
    let tools = listed(&["read"]);
    let new = |state| SessionDescription::new("m5-p08", &work, tools.clone(), LIMIT, state);
    // Не проверено после загрузки конфигурации — это отсутствие данных, а не «выключено».
    match new(CodemodeState::Unverified) {
        Err(IdentityError::CodemodeUnverified) => {}
        other => panic!("непроверенное выключение — отказ, а не {other:?}"),
    }
    // Подключился сам (MCP): виновник назван, иначе «выключено» и «подключилось» совпали бы.
    match new(CodemodeState::Attached { via: "mcp" }) {
        Err(IdentityError::CodemodeAttached { via }) => assert_eq!(via, "mcp"),
        other => panic!("codemode после конфигурации — отказ, а не {other:?}"),
    }
    // Проверенное выключение — единственный путь, и в сессии codemode действительно нет.
    let session = new(CodemodeState::AbsentAfterConfigLoad).expect("выключено проверено");
    assert!(!session.tools().holds(CODEMODE));
    // Без идентификатора сессию не собрать: «работал кто-то без имени» при разборе не
    // отличить от «не работал никто» (закон 6).
    let unnamed = CodemodeState::AbsentAfterConfigLoad;
    match SessionDescription::new(" ", &work, tools, LIMIT, unnamed) {
        Err(IdentityError::Unnamed) => {}
        other => panic!("без идентификатора — именно отказ об имени, а не {other:?}"),
    }
}

/// forbidden: путь вне рабочего каталога не проходит — после нормализации `..`, лексически
/// и без ФС. Контроль обязателен: путь ВНУТРИ проходит, иначе «отказ» выдала бы фикстура,
/// отказавшая бы вообще всему.
#[test]
fn a_path_that_leaves_the_working_directory_does_not_pass_while_one_inside_does() {
    let work = work_root("inside");
    let session = session_in(&work, &listed(&["read"]));
    let inside = work.join("nested/state.txt");
    assert_eq!(session.inside(&inside).expect("внутри"), inside.as_path());

    // Назван нормализованный путь, а не подставленный на вход: `..` уже разложен.
    let outside = work.parent().expect("корень").join("outside.txt");
    // Ожидание ВЫВОДИТСЯ из каталога фикстуры, а не записано константой: глубина
    // `temp_dir` — свойство машины, и замер `cargo mutants` подменяет `TMPDIR`. Константа
    // «/stray» упала бы там на НЕВЕРНОМ ожидании, и отказ выдала бы фикстура.
    let up = work
        .parent()
        .and_then(Path::parent)
        .expect("два уровня вверх")
        .join("stray");
    let escapes = [
        (work.join("../outside.txt"), outside.clone()),
        (work.join("nested/../../outside.txt"), outside),
        (
            Path::new("/etc/shadow").to_path_buf(),
            PathBuf::from("/etc/shadow"),
        ),
        // `..` вверх за корень песочницы не поднимается, и выход всё равно запрещён.
        (work.join("..").join("..").join("stray"), up),
    ];
    for (escape, normalised) in escapes {
        match session.inside(&escape) {
            Err(IdentityError::Outside { root, path }) => {
                assert_eq!(root, work, "корень назван, а не подставлен заново");
                assert_eq!(path, normalised, "виновник назван ПОСЛЕ `..`");
            }
            other => panic!("путь снаружи — отказ, а не {other:?}"),
        }
    }
}
