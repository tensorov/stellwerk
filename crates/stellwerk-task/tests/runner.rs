//! RED-спецификация раннера: JSON-поток Pi становится доменными событиями.
//!
//! Вердикт различает ПРИЧИНУ отказа: `InvalidLoop`, `InvalidNoToolSteps`,
//! `InvalidTooFewTurns` — разные состояния, а не один флаг (закон 6). Пороги —
//! умолчания `scripts/worker/dispatch.conf:73–75`.
//!
//! Исполнитель ПОДДЕЛЬНЫЙ (packet-09, п. 3): Pi не зовётся, строки задаёт тест, а
//! строка не-JSON обязана стать `MalformedLine`. Импорт — публичный API, не `#[path]`:
//! тот собрал бы вторую копию модуля и оставил реэкспорт непроверенным (F-14).

use stellwerk_task::runner::{verdict_for, Progress, StopReason, Verdict};
use stellwerk_task::runner::{EventSource, RunnerError};

/// Строки настоящего потока Pi: формат из `scripts/worker/tests/run.sh:41-60`.
const SESSION: &str =
    r#"{"type":"session","version":3,"id":"fix","timestamp":"2026-09-27T00:00:00Z"}"#;
const TURN_END: &str = r#"{"type":"turn_end","toolResults":[],"message":{"stopReason":"stop","usage":{"input":1000,"cacheRead":9000,"cacheWrite":0}}}"#;
const NOT_JSON: &str = "{не json";

#[test]
fn clean_run_with_one_step_and_two_turns_is_valid() {
    assert_eq!(verdict_for(StopReason::Stop, 1, 2), Verdict::Valid);
}

#[test]
fn error_stop_is_invalid_because_stop_is_not_ok() {
    let v = verdict_for(StopReason::Error, 1, 2);
    assert_eq!(v, Verdict::InvalidLoop);
    assert_ne!(v, Verdict::InvalidNoToolSteps);
}

#[test]
fn zero_tool_steps_is_its_own_reason() {
    let bad = verdict_for(StopReason::Stop, 0, 2);
    assert_eq!(bad, Verdict::InvalidNoToolSteps, "ни одного действия");
}

#[test]
fn one_turn_is_its_own_reason() {
    let v = verdict_for(StopReason::Stop, 1, 1);
    assert_eq!(v, Verdict::InvalidTooFewTurns);
}

#[test]
fn each_turn_end_adds_exactly_one_step() {
    let mut p = Progress::default();
    p.absorb(SESSION).expect("сессия — не ошибка");
    assert_eq!(p.tool_steps, 0, "сессия шагом не считается");
    p.absorb(TURN_END).expect("конец хода разбирается");
    assert_eq!(p.tool_steps, 1, "первый turn_end даёт один шаг");
    p.absorb(TURN_END).expect("конец хода разбирается");
    assert_eq!(p.tool_steps, 2, "второй даёт ещё шаг, а не снова один");
}

#[test]
fn clean_stream_through_fake_executor_counts_its_steps() {
    let mut p = Progress::default();
    let mut f = Fake::of(&[SESSION, TURN_END, TURN_END]);
    p.drain(&mut f).expect("чистый поток читается");
    assert_eq!(p.tool_steps, 2, "по шагу на каждый turn_end");
    assert_eq!(p.stop_reason, Some(StopReason::Stop), "причина названа");
}

#[test]
fn malformed_line_is_its_own_state_and_does_not_vanish() {
    let mut p = Progress::default();
    let mut f = Fake::of(&[TURN_END, NOT_JSON, TURN_END]);
    let refused = p.drain(&mut f).expect_err("строка видна");
    match refused {
        RunnerError::MalformedLine { line_no, line } => {
            assert_eq!(line_no, 2, "битая — вторая, нумерация с единицы");
            assert_eq!(line, NOT_JSON, "текст сохранён: терять его нельзя");
        }
        other => panic!("не-JSON — не отказ песочницы: {other:?}"),
    }
    assert_eq!(p.tool_steps, 1, "шаг до битой засчитан; пропуск дал бы 2");
}

struct Fake(std::vec::IntoIter<String>);

impl Fake {
    fn of(lines: &[&str]) -> Self {
        let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        Self(owned.into_iter())
    }
}

impl EventSource for Fake {
    fn next_line(&mut self) -> Option<String> {
        self.0.next()
    }
}
