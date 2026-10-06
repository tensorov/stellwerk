//! Спецификация M5/packet-08: идентичность сессии и разрешённые инструменты.
//!
//! Проверяет не состав описания, а отказ: субагент не получает ничего сверх родителя,
//! обрезка набора — ошибка, а не укороченный список, а «codemode выключен» считается
//! доказанным только после загрузки конфигурации. Фикстура ничего не создаёт и не запускает,
//! поэтому отказ здесь не может выдать сама фикстура — как и в `sandbox_bwrap.rs`.

use std::path::PathBuf;
use std::time::Duration;
use stellwerk_task::m5::session_identity::{
    CodemodeState, IdentityError, SessionDescription, ToolSet,
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
