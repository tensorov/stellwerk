//! # Цепочка доказательств и этапный gate
//!
//! `GateResult.passed` решает, считается ли пакет выполненным, — то есть эта
//! цепочка и есть то, ради чего пишут тесты. Поэтому объявляется она раньше
//! реализации: падающий тест, ссылающийся на несуществующий модуль, не
//! компилируется, а компиляционная ошибка ничего не говорит о гейте.
//!
//! Типы `EvidenceKind`/`EvidenceEntry`/`GateResult` объявлены в
//! `stellwerk_core::contract` и здесь ТОЛЬКО реэкспортируются: второе объявление
//! завело бы два определения одного понятия, и они разошлись бы молча, а
//! расхождение обнаружилось бы в чужом коде.
//!
//! GREEN M2/packet-12c: гейт считает три условия по цепочке, хвост — SHA-256 без
//! времени и случайности.

use crate::runner::{RunOutcome, Verdict};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use stellwerk_core::contract::{EvidenceEntry, EvidenceKind, GateResult};

/// Зерно хвоста: без него genesis совпал бы с «цепочкой из одной произвольной
/// записи», а хвост, который можно выдать за genesis, не защищает ничего.
const CHAIN_TAIL_SEED: &str = "stellwerk-evidence-chain-v1";

/// Заглушка хвоста времён RED (M2/packet-12b). `evaluate` её НЕ возвращает: она
/// остаётся эталоном, с которым тест сравнивает настоящий хвост, иначе сравнивать
/// было бы не с чем. Именованная константа, а не пустая строка — пустая означала бы
/// «не посчитали», а такого состояния у хеша не бывает.
pub const CHAIN_TAIL_PENDING: &str = "unimplemented-sha2-m2-packet-12c";

/// Отказ записи в цепочку. Отдельный тип, а не `()`: упавшая отправка и принятая
/// запись — разные наблюдения, и их слияние выдало бы отказ за успех (закон 6).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LedgerError {
    /// Цепочка недоступна: сеть или хранилище не ответили. Причина сохранена
    /// дословно, иначе расследование отказа получает выдуманный ответ.
    #[error("evidence ledger unavailable: {reason}")]
    Unavailable { reason: String },
}

/// Цепочка доказательств как граница записи. Сеть появится в более позднем пакете,
/// а пока трейт не даёт выдумать второе место, где записи живут.
///
/// `ModelClaim` пишет модель, и команда у него пуста. Запрет команды живёт в
/// конструкторе `model_claim`, где `command` — не параметр, а константа: параметр
/// мимо конструировал бы удостоверение, которого никто не производил.
pub trait EvidenceLedger {
    /// Принять запись. Отказ возвращается, а не теряется: запись, о которой
    /// никто не знает, и запись, которую отвергли, выглядят одинаково.
    fn append(&mut self, entry: &EvidenceEntry) -> Result<(), LedgerError>;

    /// Записи задачи в порядке записи: хвост, который не зависит от порядка, не
    /// защищает от перестановки доказательств.
    fn entries(&self, task_id: &str) -> Vec<EvidenceEntry>;
}

/// Запись `ModelClaim` — единственный способ её собрать. Команда не ПРИНИМАЕТСЯ:
/// пустая строка здесь — факт («инструмент не запускался»), а параметр сделал бы
/// её правдоподобной подделкой в одну строку (закон 1).
pub fn model_claim(task_id: &str, id: &str, result_hash: &str, ts: &str) -> EvidenceEntry {
    EvidenceEntry {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        kind: EvidenceKind::ModelClaim,
        command: String::new(),
        result_hash: result_hash.to_owned(),
        ts: ts.to_owned(),
    }
}

/// Решение этапного gate'а: прогон плюс цепочка, из которой оно выводится.
///
/// `RunOutcome.verdict` читается КАК ДАННОЕ и не пересчитывается: раннер и
/// `validity_gate.py` расходились в предикате остановки (F-2026-09-29-20), и
/// пересчёт внутри гейта молча выбрал бы сторону в открытом споре.
pub fn evaluate(outcome: &RunOutcome, chain: &[EvidenceEntry]) -> GateResult {
    let mut missing: Vec<String> = Vec::new();
    // Каждое отсутствующее условие даёт СВОЮ строку, а не одну свёрнутую «invalid»:
    // слитые причины неотличимы от одной неразобранной, и расследование получает
    // выдуманный ответ (закон 6).
    if !chain
        .iter()
        .any(|entry| entry.kind == EvidenceKind::ToolAttestation)
    {
        missing.push(
            "нет ToolAttestation: инструмент не запускался, а текст модели доказательством не является (закон 1)"
                .to_owned(),
        );
    }
    if !chain
        .iter()
        .any(|entry| entry.kind == EvidenceKind::Validation)
    {
        missing.push(
            "нет Validation: ToolAttestation никто не проверил, и выглядящий годным результат проверенным не является"
                .to_owned(),
        );
    }
    if outcome.verdict != Verdict::Valid {
        // Имя вердикта печатается, а решение не пересчитывается: спор о годности
        // прогона закрывает INFRA-03, и пересчёт здесь выбрал бы сторону за него.
        missing.push(format!(
            "вердикт прогона {:?} не Valid: годность прогона решает раннер, а не гейт",
            outcome.verdict
        ));
    }
    GateResult {
        passed: missing.is_empty(),
        missing,
        verified: chain.len() as u64,
        chain_tail_hash: chain_tail(chain),
    }
}

/// Хвост цепочки: SHA-256 от предыдущего хвоста и содержимого записи, hex в нижнем
/// регистре. Поле `ts` в хеш НЕ входит: два одинаково собранные прогона обязаны
/// дать один хвост, иначе расхождение нельзя отличить от подмены — проверяемость
/// здесь важнее уникальности.
fn chain_tail(entries: &[EvidenceEntry]) -> String {
    let mut tail = sha256_hex(CHAIN_TAIL_SEED.as_bytes());
    for entry in entries {
        let mut hasher = Sha256::new();
        hasher.update(tail.as_bytes());
        for field in [
            entry.id.as_str(),
            entry.task_id.as_str(),
            kind_name(entry.kind),
            entry.command.as_str(),
            entry.result_hash.as_str(),
        ] {
            // Длина идёт перед полем: склейка без неё не отличает ("a","bc") от
            // ("ab","c"), а это две разные цепочки с одним хвостом.
            hasher.update((field.len() as u64).to_be_bytes());
            hasher.update(field.as_bytes());
        }
        tail = format!("{:x}", hasher.finalize());
    }
    tail
}

/// Имя вида записи в хвосте — те же строки, что у PolyForge и в сериализации
/// контракта: своё имя разошлось бы с контрактом, и записи разных видов сошлись бы
/// в одну.
fn kind_name(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::ModelClaim => "ModelClaim",
        EvidenceKind::ToolAttestation => "ToolAttestation",
        EvidenceKind::Validation => "Validation",
    }
}

/// SHA-256 в hex нижнего регистра. Крейт из `[workspace.dependencies]`, а не
/// `DefaultHasher`: тот меняется между версиями rustc и даёт 64 бита — в цепочке
/// доказательств это уверенный вид подмены, которой не было.
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
