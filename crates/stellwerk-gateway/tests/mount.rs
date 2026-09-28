//! Tests for `plan` — lazy mounting: бюджет выбирает, что подключать (M3-07).
//! RED: `plan` — заглушка, восемь сценариев падают, прогон компилируется.

use stellwerk_core::contract::{ModuleKind, RiskClass};
use stellwerk_gateway::mount::{plan, Candidate, MountPlan, SkipReason, ToolRisk};

/// Модуль без risk в `modules.toml` — это `UNKNOWN`: набор тестов нигде не должен
/// читаться как «риска нет».
const R0: ToolRisk = ToolRisk::Classified(RiskClass::R0);
const R1: ToolRisk = ToolRisk::Classified(RiskClass::R1);
const UNKNOWN: ToolRisk = ToolRisk::Unclassified;

/// Кандидаты из строк `(имя, цена, риск, сдвиг риска)`: rustfmt разворачивает
/// литерал шире 18 символов в семь строк, и восемь сценариев в лимит пакета не
/// поместились бы. `risk_delta` — своя колонка, а не спрятанный `..base()`.
fn cands(rows: &[(&str, u64, ToolRisk, i64)]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::with_capacity(rows.len());
    for &(name, cost_tokens, risk, risk_delta) in rows {
        let candidate = Candidate {
            name: name.into(),
            kind: ModuleKind::Mcp,
            cost_tokens,
            risk,
            risk_delta,
        };
        out.push(candidate);
    }
    out
}

/// План по бюджету: планирование не падает ни на одном сценарии, и восемь
/// одинаковых `.expect` закрепили бы лишнее — форму `Result` как таковую.
fn planned(rows: &[(&str, u64, ToolRisk, i64)], budget: u64) -> MountPlan {
    plan(&cands(rows), budget).expect("планирование не падает")
}

#[test]
fn cheap_modules_mount_first_in_ascending_cost() {
    // Дано: поданы ДОРОГОВЫМ первым, бюджет на всё. Тогда: по возрастанию цены.
    let rows = &[("reg", 90, R1, 0), ("web", 30, R0, 0), ("api", 60, R1, 0)];
    assert_eq!(planned(rows, 300).mounted, vec!["web", "api", "reg"]);
}

#[test]
fn same_module_set_gets_the_same_budget_in_any_order() {
    // Дано: один набор двумя порядками, бюджет режет посередине. Тогда: планы
    // совпадают — иначе разница стала бы неразличимой от смены состава.
    let a = &[("web", 30, R0, 0), ("api", 60, R1, 0), ("reg", 90, R1, 0)];
    let b = &[("reg", 90, R1, 0), ("web", 30, R0, 0), ("api", 60, R1, 0)];
    assert_eq!(planned(a, 90), planned(b, 90), "один набор — один бюджет");
}

#[test]
fn over_budget_module_is_skipped_and_mounting_continues() {
    // Дано: имена в ОБРАТНОМ порядке цен (a_big дороже всех, c_cheap дешевле
    // b_base) — реализация, сортирующая по имени, провалится. Тогда: жадный
    // проход, а не отказ от набора из-за одного дорогого: 100 + 200 в бюджете.
    let rows = &[
        ("a_big", 5_000, R1, 0),
        ("b_base", 100, R0, 0),
        ("c_cheap", 200, R1, 0),
    ];
    let r = planned(rows, 400);
    assert_eq!(r.mounted, vec!["b_base", "c_cheap"]);
    assert_eq!(r.skipped, [("a_big".to_string(), SkipReason::OverBudget)]);
}

#[test]
fn unclassified_risk_is_skipped_and_never_mounts() {
    // Дано: самый дешёвый и целиком в бюджете — отсечь может ТОЛЬКО риск. Тогда:
    // В2 — «риск неизвестен» это состояние, а не нулевой риск: причина видна.
    let r = planned(&[("unknown", 10, UNKNOWN, 0)], 400);
    assert!(r.mounted.is_empty(), "не монтируется: {r:?}");
    let skipped = [("unknown".to_string(), SkipReason::Unclassified)];
    assert_eq!(r.skipped, skipped);
}

#[test]
fn unclassified_is_not_the_same_verdict_as_over_budget() {
    // Дано: два пропуска по разным причинам в одном прогоне. Тогда: «не
    // классифицирован» и «дорого» в расследовании не смешиваются.
    let r = planned(&[("unknown", 10, UNKNOWN, 0), ("huge", 500, R0, 0)], 400);
    let expected = [
        ("unknown".to_string(), SkipReason::Unclassified),
        ("huge".to_string(), SkipReason::OverBudget),
    ];
    assert_eq!(r.skipped, expected);
}

#[test]
fn r5_is_the_only_risk_cut_and_ask_survives_mounting() {
    // Дано: R4, поднятый сетью до R5; R3, поднятый до R4; PolyForge, который риск
    // СНИМАЕТ; R5 как есть. Все в бюджете — режет только риск. Тогда: R3/R4
    // монтируются, их `Ask` разбирается в `decide` (В1, В4).
    let rows = &[
        ("network", 100, ToolRisk::Classified(RiskClass::R4), 1),
        ("secrets", 200, ToolRisk::Classified(RiskClass::R3), 1),
        ("forge", 300, R1, -1),
        ("nuke", 400, ToolRisk::Classified(RiskClass::R5), 0),
    ];
    let r = planned(rows, 800);
    assert_eq!(r.mounted, vec!["secrets", "forge"]);
    let expected = [
        ("network".to_string(), SkipReason::TooRisky),
        ("nuke".to_string(), SkipReason::TooRisky),
    ];
    assert_eq!(r.skipped, expected);
}

#[test]
fn repeated_name_is_a_duplicate() {
    // Дано: два MCP с одним именем, оба в бюджете — иначе пропуск объяснила бы
    // ещё и нехватка бюджета, и тест проверял бы не то.
    let r = planned(&[("note", 100, R1, 0), ("note", 700, R1, 0)], 1_000);
    assert_eq!(r.mounted, vec!["note"]);
    assert_eq!(r.skipped, [("note".to_string(), SkipReason::Duplicate)]);
}

#[test]
fn skill_module_is_never_mounted_even_at_zero_price() {
    // Дано: Skill за 0 токенов с известным риском R0 — ни одна причина, кроме
    // Disabled, его не объясняет. Тогда: инструкция платит контекстом, а
    // монтировать нечего.
    let mut c = cands(&[("handbook", 0, R0, 0)]);
    c[0].kind = ModuleKind::Skill;
    let r = plan(&c, 400).expect("планирование не падает");
    assert!(r.mounted.is_empty(), "Skill не монтируется: {r:?}");
    assert_eq!(r.skipped, [("handbook".to_string(), SkipReason::Disabled)]);
}
