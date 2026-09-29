//! RED M2/packet-12b: что считается доказательством, а что нет.
//!
//! Четыре сценария, и каждый ломается по-своему. Одиночный `ModelClaim` не
//! доказывает ничего — закон 1: «готово» это `ToolAttestation`, а не текст модели.
//! Отсутствующие условия дают РАЗНЫЕ строки в `missing`: слитая причина
//! неотличима от одной неразобранной (закон 6). Порядок обязателен. Хвост
//! воспроизводим — иначе расхождение нельзя отличить от подмены результата.
//!
//! Импорт идёт через публичный API крейта, не `#[path]`: тот собрал бы вторую
//! копию типов контракта, и расхождение обнаружилось бы в чужом коде.

use stellwerk_task::evidence::{evaluate, EvidenceEntry, EvidenceKind, CHAIN_TAIL_PENDING};
use stellwerk_task::runner::{RunOutcome, StopReason, Verdict};

const TASK: &str = "t-evidence";
/// Время фиксировано: время внутри хвоста сделало бы тест неповторяемым, а
/// цепочка, которую нельзя воспроизвести, не удостоверяет ничего.
const TS: &str = "2026-09-29T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test evidence";

fn outcome(verdict: Verdict) -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1_000,
        verdict,
    }
}

/// Запись цепочки. Команда ВЫВОДИТСЯ из вида, а не принимается: `ModelClaim`
/// инструмент не запускал, и пустая строка здесь — факт, а не пропущенное поле.
fn entry(id: &str, kind: EvidenceKind, result_hash: &str) -> EvidenceEntry {
    EvidenceEntry {
        id: id.to_owned(),
        task_id: TASK.to_owned(),
        kind,
        command: match kind {
            EvidenceKind::ModelClaim => String::new(),
            EvidenceKind::ToolAttestation | EvidenceKind::Validation => TEST_CMD.to_owned(),
        },
        result_hash: result_hash.to_owned(),
        ts: TS.to_owned(),
    }
}

/// Цепочка из одной записи: длина в одну, доказательств ноль.
fn claim_only() -> Vec<EvidenceEntry> {
    vec![entry("e-1", EvidenceKind::ModelClaim, "h-claim")]
}

/// Две записи вместо одной: количество записей не заменяет доказательства, и без
/// `ToolAttestation`/`Validation` отсутствуют сразу ДВА условия.
fn claims_only() -> Vec<EvidenceEntry> {
    vec![
        entry("e-1", EvidenceKind::ModelClaim, "h-claim"),
        entry("e-2", EvidenceKind::ModelClaim, "h-claim-2"),
    ]
}

/// Валидация подтверждает то, чего никто не производил: её `result_hash` указывает
/// на хеш `ModelClaim`, а не на `ToolAttestation`, и выглядит убедительно.
fn validated_without_attestation() -> Vec<EvidenceEntry> {
    vec![
        entry("e-1", EvidenceKind::ModelClaim, "h-claim"),
        entry("e-2", EvidenceKind::Validation, "h-claim"),
    ]
}

/// Цепочка с удостоверением: удостоверение есть, валидации нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        entry("e-1", EvidenceKind::ModelClaim, "h-claim"),
        entry("e-2", EvidenceKind::ToolAttestation, "h-tool"),
    ]
}

/// Та же цепочка плюс валидация. Отличается от `attested` ровно одной записью, и
/// хвост обязан это видеть.
fn attested_and_validated() -> Vec<EvidenceEntry> {
    let mut chain = attested();
    chain.push(entry("e-3", EvidenceKind::Validation, "h-tool"));
    chain
}

#[test]
fn lone_model_claim_is_not_proof() {
    let gate = evaluate(&outcome(Verdict::Valid), &claim_only());
    assert!(
        !gate.passed,
        "модель не верифицирует себя: закон 1 требует ToolAttestation, а не текста модели"
    );
}

#[test]
fn two_absent_conditions_give_two_reasons() {
    let gate = evaluate(&outcome(Verdict::Valid), &claims_only());
    assert!(
        !gate.passed,
        "две записи ModelClaim — всё ещё ноль доказательств, длина цепочки её не заменяет"
    );
    assert_eq!(
        gate.missing.len(),
        2,
        "по строке на каждое отсутствующее условие, а не одна свёрнутая: {:?}",
        gate.missing
    );
    let mut uniq = gate.missing.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        2,
        "слитые причины неотличимы от одной неразобранной (закон 6): {:?}",
        gate.missing
    );
    assert!(
        gate.missing.iter().all(|reason| !reason.trim().is_empty()),
        "пустая строка причины значит «не посчитали», а у причины такого состояния нет: {:?}",
        gate.missing
    );
}

#[test]
fn validation_without_tool_attestation_does_not_pass() {
    let gate = evaluate(&outcome(Verdict::Valid), &validated_without_attestation());
    assert!(
        !gate.passed,
        "валидация, подтверждающая несуществующее доказательство, не доказывает ничего, \
         даже если её result_hash выглядит убедительно"
    );
    assert_eq!(
        gate.missing.len(),
        1,
        "валидация условие выполняет — отсутствует ровно одно, и коллапс двух причин \
         в одну выдал бы ложную полноту: {:?}",
        gate.missing
    );
}

#[test]
fn chain_tail_is_reproducible_and_follows_the_chain() {
    let run = outcome(Verdict::Valid);
    let once = evaluate(&run, &attested());
    let twice = evaluate(&run, &attested());
    assert_eq!(
        once.chain_tail_hash, twice.chain_tail_hash,
        "две одинаково собранные цепочки обязаны дать один хвост: иначе расхождение \
         нельзя отличить от подмены"
    );
    assert_ne!(
        once.chain_tail_hash, CHAIN_TAIL_PENDING,
        "заглушка, выданная за хвост, — уверенный вид доказательства, которого нет"
    );
    let longer = evaluate(&run, &attested_and_validated());
    assert_ne!(
        once.chain_tail_hash, longer.chain_tail_hash,
        "хвост обязан зависеть от состава цепочки: иначе добавленная запись не видна за ним"
    );
}
