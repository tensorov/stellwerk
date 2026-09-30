//! RED M2/packet-11: шаг Pi становится событием федерации.
//!
//! Круг «записать → прочитать» обязателен: `append` выносит `payload["task_id"]` в
//! колонку, и `list_by_task` выбирает по ней — только он доказывает форму payload.
//! Причины отказа различаются ПО ИМЕНИ (`loop`/`no_tool_steps`/`too_few_turns`) —
//! склейка отказов потеряла бы ровно то, ради чего журнал существует (закон 6).
//! Импорт идёт через публичный API крейта, не `#[path]`: тот собрал бы вторую копию.

use std::path::Path;

use stellwerk_core::contract::{
    DataClass, Event, EventRisk, EventSource, RiskClass, TaskLimits, TaskSpec,
};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::events::{events_for, Step, StepKind, Verdict};
use stellwerk_task::runner::{RunOutcome, StopReason};

fn spec() -> TaskSpec {
    TaskSpec {
        id: "t-events".to_owned(),
        goal: "Записать шаги прогона в журнал".to_owned(),
        workspace_ref: "ws-stellwerk".to_owned(),
        model_ref: "qwen-local".to_owned(),
        packet: "сделай ровно одну вещь".to_owned(),
        limits: TaskLimits {
            max_files: 3,
            max_diff_lines: 200,
            timeout_seconds: 900,
            max_retries: 2,
            context_budget_tokens: 40_000,
            data_class: DataClass::Internal,
        },
        evidence_policy: "cargo test -p stellwerk-task --test events".to_owned(),
    }
}

fn outcome(verdict: Verdict) -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1000,
        verdict,
    }
}

fn step(kind: StepKind, tool: &str) -> Step {
    let tool = tool.to_owned();
    Step { kind, tool }
}

fn steps() -> [Step; 2] {
    let (net, file) = (StepKind::Network, StepKind::FileWrite);
    [step(file, "edit"), step(net, "fetch")]
}

fn of_type<'a>(events: &'a [Event], event_type: &str) -> Vec<&'a Event> {
    events
        .iter()
        .filter(|e| e.event_type == event_type)
        .collect()
}

fn types_of(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.event_type.as_str()).collect()
}

#[test]
fn every_step_yields_one_run_step_with_its_own_risk() {
    let events = events_for(&spec(), &outcome(Verdict::Valid), &steps());
    let risks: Vec<EventRisk> = of_type(&events, "run.step")
        .iter()
        .map(|e| e.risk)
        .collect();
    let why = "правка — R1, сеть — R2, по событию на шаг: иначе выход скрыт";
    assert_eq!(
        risks,
        vec![
            EventRisk::Classified(RiskClass::R1),
            EventRisk::Classified(RiskClass::R2)
        ],
        "{why}"
    );
    let end = of_type(&events, "run.end");
    let end = end.first().unwrap_or_else(|| panic!("нет run.end"));
    assert_eq!(
        end.risk,
        EventRisk::Classified(RiskClass::R2),
        "конец — максимум шагов, не первый"
    );
}

#[test]
fn run_opens_with_start_at_r1_and_stamps_rfc3339_utc() {
    let events = events_for(&spec(), &outcome(Verdict::Valid), &steps());
    let start = of_type(&events, "run.start");
    let start = start.first().unwrap_or_else(|| panic!("нет run.start"));
    assert_eq!(
        start.risk,
        EventRisk::Classified(RiskClass::R1),
        "старт прогона — R1"
    );
    let ts = start.ts.clone();
    let utc = ts.ends_with('Z') || ts.ends_with("+00:00");
    assert!(
        utc,
        "ts обязан быть RFC 3339 UTC, а не местным временем: {ts}"
    );
}

#[test]
fn every_invalid_reason_has_its_own_payload_name() {
    for (verdict, reason) in [
        (Verdict::InvalidLoop, "loop"),
        (Verdict::InvalidNoToolSteps, "no_tool_steps"),
        (Verdict::InvalidTooFewTurns, "too_few_turns"),
    ] {
        let events = events_for(&spec(), &outcome(verdict), &[]);
        let bad = of_type(&events, "run.invalid");
        let bad = bad.first().unwrap_or_else(|| panic!("{verdict:?} — нет"));
        assert_eq!(
            bad.risk,
            EventRisk::Classified(RiskClass::R1),
            "{verdict:?} — R1"
        );
        let named = bad.payload.get("reason").map(String::as_str);
        assert_eq!(named, Some(reason), "{verdict:?} дал {:?}", bad.payload);
    }
}

#[test]
fn valid_run_writes_no_run_invalid() {
    let events = events_for(&spec(), &outcome(Verdict::Valid), &steps());
    let bad = of_type(&events, "run.invalid");
    assert!(bad.is_empty(), "валидный прогон — не отказ: {events:?}");
}

#[test]
fn every_event_names_its_task_and_workspace() {
    let spec = spec();
    let events = events_for(&spec, &outcome(Verdict::InvalidLoop), &steps());
    for e in &events {
        let task = e.payload.get("task_id").map(String::as_str);
        let ws = e.workspace.as_deref();
        let src = e.source;
        assert_eq!(task, Some(spec.id.as_str()), "без task_id: {e:?}");
        assert_eq!(ws, Some("ws-stellwerk"), "место из спецификации");
        assert_eq!(src, EventSource::System, "федерация, не человек");
    }
}

#[tokio::test]
async fn events_reach_the_store_and_come_back_in_write_order() {
    let store = SqliteStore::open(Path::new(":memory:")).expect("store must open");
    let spec = spec();
    let events = events_for(&spec, &outcome(Verdict::Valid), &steps());
    for e in &events {
        store.append(e.clone()).await.expect("append must succeed");
    }
    let mine = store.list_by_task(&spec.id).await.expect("list");
    let want = types_of(&events);
    assert_eq!(mine.len(), events.len(), "выборка не теряет события");
    assert_eq!(types_of(&mine), want, "порядок записи выживает");
}
