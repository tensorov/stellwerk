//! Перечень девяти обязательных отказов — как данные (M5/packet-10, п. 3).
//!
//! Список закрыт: семь отказов владельца плюс два из пробы Pi 1.0.2.

/// Число отказов — в ТИПЕ массива: удаление элемента ломает сборку.
pub const NINE: usize = 9;

/// Пара `(имя проверки, имя варианта отказа)`: имя хранится данными, а не
/// вычисляется из отказа. Сверяется префиксом `Debug` — он равен имени варианта.
pub type Boundary = (&'static str, &'static str);

pub const ROSTER: [Boundary; NINE] = [
    ("write_outside", "Outside"),
    ("token_in_agent_env", "TokenInEnvironment"),
    ("self_approval", "Transition(Suspended)"),
    ("subagent_escalation", "Escalation"),
    ("publish_unanswered", "Suspended"),
    ("content_not_swapped", "Truncated"),
    ("engine_unavailable", "Spawn"),
    ("mcp_config_detached", "NoSuchTool"),
    ("codemode_recognised", "UnrecognisedSource"),
];

/// Отказ НАЗВАН и виновник назван; пустой текст — падение (п. 2 пакета).
pub fn assert_named(id: &str, observed: &str) {
    let found = ROSTER.iter().find(|(name, _)| *name == id);
    let Some(want) = found.map(|(_, why)| *why) else {
        panic!("проверки `{id}` нет в перечне")
    };
    assert!(!observed.trim().is_empty(), "{id}: ПУСТОЙ отказ");
    let ok = observed.starts_with(want);
    assert!(ok, "{id}: ждали начало {want:?}, получили {observed:?}");
    assert!(observed.len() > want.len(), "{id}: виновник не назван");
}

/// П. 3: перечень не уменьшился и совпадает с набором проверок.
#[test]
fn the_roster_has_not_shrunk() {
    let ids: Vec<&str> = ROSTER.iter().map(|(name, _)| *name).collect();
    let distinct = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), NINE, "имена повторяются");
    let source = include_str!("negative_boundaries.rs");
    for (name, refusal) in ROSTER.iter() {
        assert!(!refusal.trim().is_empty(), "{name}: отказ не назван");
        assert!(source.contains(name), "{name}: проверки нет в наборе");
    }
}

/// П. 2 под отрицательным управлением: пустой отказ обязан ронять проверку.
#[test]
fn an_empty_refusal_is_not_taken_for_a_success() {
    let outcome = std::panic::catch_unwind(|| assert_named("write_outside", " "));
    assert!(outcome.is_err(), "пустой отказ прошёл как успех");
}
