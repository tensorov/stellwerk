//! RED M2/packet-12b: что считается доказательством, а что нет.
//!
//! Четыре сценария, и каждый ломается по-своему. Одиночный `ModelClaim` не
//! доказывает ничего — закон 1: «готово» это `ToolAttestation`, а не текст модели.
//! Отсутствующие условия дают РАЗНЫЕ строки в `missing`: слитая причина
//! неотличима от одной неразобранной (закон 6). Порядок обязателен. Хвост
//! воспроизводим — иначе расхождение нельзя отличить от подмены результата.
//!
//! GREEN N1/packet-07: записи собираются ТОЛЬКО конструкторами. Тестовая
//! `Validation` помечена меткой фикстуры и решением человека не считается
//! (решение 5).
//!
//! Импорт идёт через публичный API крейта, не `#[path]`: тот собрал бы вторую
//! копию типов контракта, и расхождение обнаружилось бы в чужом коде.

use stellwerk_task::evidence::{
    evaluate, evidence_status, model_claim, tool_attestation, validation, EvidenceEntry,
    EvidenceKind, EvidenceStatus, CHAIN_TAIL_PENDING, FIXTURE_VALIDATION_LABEL,
};
use stellwerk_task::runner::{RunOutcome, StopReason, Verdict};

const TASK: &str = "t-evidence";
/// Время фиксировано: время внутри хвоста сделало бы тест неповторяемым, а
/// цепочка, которую нельзя воспроизвести, не удостоверяет ничего.
const TS: &str = "2026-09-29T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test evidence";
/// Предмет человеческого решения: кто и что решил. Кладётся в `command` записи
/// `Validation` — отдельного поля «кто проверял» у `EvidenceEntry` нет.
const HUMAN_SUBJECT: &str = "owner reviewed the diff and accepted";

fn outcome(verdict: Verdict) -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1_000,
        verdict,
    }
}

fn claim(id: &str, result_hash: &str) -> EvidenceEntry {
    model_claim(TASK, id, result_hash, TS)
}

/// Цепочка из одной записи: длина в одну, доказательств ноль.
fn claim_only() -> Vec<EvidenceEntry> {
    vec![claim("e-1", "h-claim")]
}

/// Две записи вместо одной: количество записей не заменяет доказательства, и без
/// `ToolAttestation`/`Validation` отсутствуют сразу ДВА условия.
fn claims_only() -> Vec<EvidenceEntry> {
    vec![claim("e-1", "h-claim"), claim("e-2", "h-claim-2")]
}

/// Валидация подтверждает то, чего никто не производил: её `result_hash` указывает
/// на хеш `ModelClaim`, а не на `ToolAttestation`, и выглядит убедительно.
fn validated_without_attestation() -> Vec<EvidenceEntry> {
    vec![
        claim("e-1", "h-claim"),
        validation(TASK, "e-2", TEST_CMD, "h-claim", TS),
    ]
}

/// Цепочка с удостоверением: удостоверение есть, валидации нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        claim("e-1", "h-claim"),
        tool_attestation(TASK, "e-2", TEST_CMD, "h-tool", TS),
    ]
}

/// Та же цепочка плюс валидация. Отличается от `attested` ровно одной записью, и
/// хвост обязан это видеть.
fn attested_and_validated() -> Vec<EvidenceEntry> {
    let mut chain = attested();
    chain.push(validation(TASK, "e-3", TEST_CMD, "h-tool", TS));
    chain
}

/// Цепочка с ЧЕЛОВЕЧЕСКОЙ валидацией: субъект решения — не метка фикстуры.
fn attested_and_human_validated() -> Vec<EvidenceEntry> {
    let mut chain = attested();
    chain.push(validation(TASK, "e-human", HUMAN_SUBJECT, "h-tool", TS));
    chain
}

/// Все три вида на месте, но удостоверение записано раньше утверждения модели.
/// Присутствие звеньев не заменяет их порядок: перестановка доказательств — это
/// другая цепочка.
fn wrong_order() -> Vec<EvidenceEntry> {
    vec![
        tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS),
        claim("e-claim", "h-claim"),
        validation(TASK, "e-validation", TEST_CMD, "h-tool", TS),
    ]
}

fn assert_entry(
    entry: &EvidenceEntry,
    id: &str,
    kind: EvidenceKind,
    command: &str,
    result_hash: &str,
) {
    assert_eq!(entry.id, id, "id записи");
    assert_eq!(entry.task_id, TASK, "задача записи");
    assert_eq!(entry.kind, kind, "вид записи");
    assert_eq!(entry.command, command, "команда записи");
    assert_eq!(entry.result_hash, result_hash, "хеш результата");
    assert_eq!(entry.ts, TS, "время записи");
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
fn correct_order_closes_the_gate() {
    let gate = evaluate(&outcome(Verdict::Valid), &attested_and_validated());
    assert!(
        gate.passed,
        "цепочка ModelClaim → ToolAttestation → Validation обязана закрывать гейт: {:?}",
        gate.missing
    );
    assert!(
        gate.missing.is_empty(),
        "у закрытого гейта нет причин отказа: {:?}",
        gate.missing
    );
}

#[test]
fn wrong_order_is_refused_and_names_the_expected_order() {
    let gate = evaluate(&outcome(Verdict::Valid), &wrong_order());
    assert!(
        !gate.passed,
        "все три вида на месте, но перестановка звеньев — это другая цепочка: {:?}",
        gate.missing
    );
    assert!(
        gate.missing
            .iter()
            .any(|reason| reason.contains("ModelClaim")
                && reason.contains("ToolAttestation")
                && reason.contains("Validation")),
        "причина обязана назвать ожидаемый порядок, а не просто «неверно»: {:?}",
        gate.missing
    );
    assert!(
        gate.missing.iter().all(|reason| !reason.trim().is_empty()),
        "пустая строка причины значит «не посчитали»: {:?}",
        gate.missing
    );
}

#[test]
fn tool_attestation_without_command_is_refused() {
    let chain = vec![
        claim("e-1", "h-claim"),
        tool_attestation(TASK, "e-2", "", "h-tool", TS),
        validation(TASK, "e-3", TEST_CMD, "h-tool", TS),
    ];
    let gate = evaluate(&outcome(Verdict::Valid), &chain);
    assert!(
        !gate.passed,
        "удостоверение без команды не называет записанный запуск: {:?}",
        gate.missing
    );
    assert!(
        gate.missing.iter().any(|reason| reason.contains("команд")),
        "отказ обязан назвать пустую команду, а не молчать: {:?}",
        gate.missing
    );
    assert!(
        gate.missing.iter().all(|reason| !reason.trim().is_empty()),
        "пустая строка причины значит «не посчитали»: {:?}",
        gate.missing
    );
}

#[test]
fn present_entry_without_digest_is_refused() {
    let chain = vec![
        claim("e-1", ""),
        tool_attestation(TASK, "e-2", TEST_CMD, "h-tool", TS),
        validation(TASK, "e-3", TEST_CMD, "h-tool", TS),
    ];
    let gate = evaluate(&outcome(Verdict::Valid), &chain);
    assert!(
        !gate.passed,
        "звено без digest не удостоверяет, что результат не подменён: {:?}",
        gate.missing
    );
    assert!(
        gate.missing
            .iter()
            .any(|reason| reason.contains("digest") && reason.contains("e-1")),
        "отказ обязан назвать запись без digest: {:?}",
        gate.missing
    );
    assert!(
        gate.missing.iter().all(|reason| !reason.trim().is_empty()),
        "пустая строка причины значит «не посчитали»: {:?}",
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

#[test]
fn chain_tail_distinguishes_entry_kinds() {
    let run = outcome(Verdict::Valid);
    let claim = model_claim(TASK, "e-1", "h", TS);
    let tool = tool_attestation(TASK, "e-1", "", "h", TS);
    let claim_tail = evaluate(&run, &[claim]).chain_tail_hash;
    let tool_tail = evaluate(&run, &[tool]).chain_tail_hash;
    assert_ne!(
        claim_tail, tool_tail,
        "вид записи обязан входить в хвост: иначе ModelClaim и ToolAttestation \
         с одинаковыми прочими полями сойдутся в один хвост"
    );
}

#[test]
fn empty_chain_tail_is_a_real_digest_not_a_placeholder() {
    let gate = evaluate(&outcome(Verdict::Valid), &[]);
    assert_ne!(
        gate.chain_tail_hash, CHAIN_TAIL_PENDING,
        "заглушка, выданная за хвост, — доказательство, которого не считали"
    );
    assert_eq!(
        gate.chain_tail_hash.len(),
        64,
        "хвост genesis — SHA-256 в hex, а не догадка: {:?}",
        gate.chain_tail_hash
    );
    assert!(
        gate.chain_tail_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "хвост — hex нижнего регистра: {:?}",
        gate.chain_tail_hash
    );
}

#[test]
fn constructors_set_every_field() {
    let cause = claim("e-1", "h-claim");
    assert_entry(&cause, "e-1", EvidenceKind::ModelClaim, "", "h-claim");

    let tool = tool_attestation(TASK, "e-2", TEST_CMD, "h-tool", TS);
    assert_entry(
        &tool,
        "e-2",
        EvidenceKind::ToolAttestation,
        TEST_CMD,
        "h-tool",
    );

    let human = validation(TASK, "e-3", HUMAN_SUBJECT, "h-tool", TS);
    assert_entry(
        &human,
        "e-3",
        EvidenceKind::Validation,
        HUMAN_SUBJECT,
        "h-tool",
    );
}

#[test]
fn tool_attestation_records_the_factual_verification() {
    let tool = tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS);
    assert_eq!(tool.kind, EvidenceKind::ToolAttestation);
    assert_eq!(
        tool.command, TEST_CMD,
        "записанный запуск и есть доказательство: без команды это не удостоверение"
    );
    assert_eq!(
        tool.result_hash, "h-tool",
        "зафиксирован хеш результата, а не «функция вернула Ok»"
    );
    let cause = claim("e-claim", "h-claim");
    assert_eq!(
        cause.command, "",
        "ModelClaim не несёт команды: инструмент не запускался (закон 1)"
    );
    assert_ne!(
        cause.kind,
        EvidenceKind::ToolAttestation,
        "текст модели не превращается в удостоверение"
    );
}

#[test]
fn only_the_constructors_mint_entries() {
    let source = include_str!("../src/evidence.rs");
    assert_eq!(
        source.matches("kind: EvidenceKind::").count(),
        3,
        "структурный литерал EvidenceEntry собирается ровно в трёх конструкторах; \
         четвёртая точка встречи — второй путь построения"
    );
    for kind in ["ModelClaim", "ToolAttestation", "Validation"] {
        assert_eq!(
            source
                .matches(&format!("kind: EvidenceKind::{kind}"))
                .count(),
            1,
            "вид {kind} выставляется ровно одним конструктором"
        );
    }
}

#[test]
fn human_validation_accepts_the_chain() {
    let chain = attested_and_human_validated();
    assert_eq!(
        evidence_status(&chain),
        EvidenceStatus::HumanValidated,
        "человеческая Validation — явное решение, и только оно делает результат принятым"
    );
    assert_eq!(
        EvidenceStatus::HumanValidated.label(),
        "принято человеком",
        "метка принятого состояния читается, а не выводится из пустоты"
    );
}

#[test]
fn chain_without_validation_waits_for_a_human_and_is_not_done() {
    let status = evidence_status(&attested());
    assert_eq!(
        status,
        EvidenceStatus::AwaitingHumanValidation,
        "удостоверение есть, решения человека нет — результат ждёт человека"
    );
    assert_eq!(status.label(), "ждёт человека");
    assert_ne!(
        status,
        EvidenceStatus::HumanValidated,
        "без Validation результат не принят: отсутствие решения и решение неотличимы не должны быть"
    );
    assert_ne!(
        status.label(),
        "Done",
        "Done не объявляется до решения человека (решение 5)"
    );
    assert_eq!(
        evidence_status(&claim_only()),
        EvidenceStatus::Unattested,
        "без ToolAttestation ждать нечего: это не «ждёт человека», а «предъявлять нечего»"
    );
    assert_eq!(EvidenceStatus::Unattested.label(), "нет удостоверения");
}

#[test]
fn fixture_validation_is_labelled_and_not_a_human_decision() {
    let mut chain = attested();
    chain.push(validation(
        TASK,
        "e-fixture",
        FIXTURE_VALIDATION_LABEL,
        "h-tool",
        TS,
    ));
    let fixture = chain.last().expect("запись добавлена");
    assert_eq!(
        fixture.command, FIXTURE_VALIDATION_LABEL,
        "фикстура помечена явно, а не выдана за решение человека"
    );
    assert!(
        fixture.command.contains("fixture"),
        "метка читается как фикстура: {fixture:?}"
    );
    assert_eq!(
        evidence_status(&chain),
        EvidenceStatus::AwaitingHumanValidation,
        "фикстура — не решение человека: тест не выдаёт себя за приёмку (решение 5)"
    );
}
