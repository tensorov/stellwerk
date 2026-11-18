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
use stellwerk_core::contract::TaskStatus;
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
        recipe_digest: None,
        policy_digest: None,
        model_fingerprint: None,
        commit_sha: None,
    }
}

/// Запись `ToolAttestation` — доказательство того, что инструмент ДЕЙСТВИТЕЛЬНО
/// запускался и его результат зафиксирован. В отличие от `model_claim`, команда
/// здесь ПРИНИМАЕТСЯ: записанный запуск и есть доказательство, и без него
/// удостоверения нет — пустая команда означала бы «инструмент не запускался», то
/// есть утверждение модели, а не проверку (закон 1).
///
/// Конструктор authority НЕ является (решение 5): он не решает, была ли проверка
/// релевантной, — это делает вызывающий (`orchestrator::execute`), и отвечает за
/// это он же. Здесь фиксируется форма факта, а не право объявить результат годным.
pub fn tool_attestation(
    task_id: &str,
    id: &str,
    command: &str,
    result_hash: &str,
    ts: &str,
) -> EvidenceEntry {
    EvidenceEntry {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        kind: EvidenceKind::ToolAttestation,
        command: command.to_owned(),
        result_hash: result_hash.to_owned(),
        ts: ts.to_owned(),
        recipe_digest: None,
        policy_digest: None,
        model_fingerprint: None,
        commit_sha: None,
    }
}

/// Метка тестовой `Validation`. Именованная константа, а не пустая строка:
/// фикстура, неотличимая от решения человека, выдавала бы тест за приёмку
/// (решение 5). `evidence_status` по этой метке отличает фикстуру от человеческого
/// решения, поэтому тестовая `Validation` допустима только с ней в `subject`.
pub const FIXTURE_VALIDATION_LABEL: &str = "fixture: not a human decision";

/// Запись `Validation` — ОТДЕЛЬНОЕ, ЯВНОЕ решение человека. `subject` — кто и как
/// решил; он кладётся в `command`, потому что поля «кто проверял» у `EvidenceEntry`
/// по-прежнему нет: P03 добавил bindings, но не автора решения.
///
/// Тестовая `Validation` допустима только как явно помеченная фикстура — с
/// `FIXTURE_VALIDATION_LABEL` в `subject`; `evidence_status` такую запись решением
/// человека не считает.
pub fn validation(
    task_id: &str,
    id: &str,
    subject: &str,
    result_hash: &str,
    ts: &str,
) -> EvidenceEntry {
    EvidenceEntry {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        kind: EvidenceKind::Validation,
        command: subject.to_owned(),
        result_hash: result_hash.to_owned(),
        ts: ts.to_owned(),
        recipe_digest: None,
        policy_digest: None,
        model_fingerprint: None,
        commit_sha: None,
    }
}

/// Почему опровержение отвергнуто. Отдельный тип, а не `()`: у опровержения без
/// основания и у опровержения без ссылки на данные РАЗНЫЕ причины, и слитая
/// причина неотличима от неразобранной (закон 6).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RefutationError {
    /// Основание не названо. Опровержение без основания неотличимо от отказа без
    /// причины, поэтому это отказ, а не молчаливый дефолт.
    #[error(
        "refutation without a stated basis: a refutation must name why the evidence is refuted"
    )]
    MissingBasis,
    /// Ссылка на опровергающие данные не названа. Проверять опровержение не по
    /// чему, поэтому запись не принимается.
    #[error("refutation without a reference to refuting data: the refutation must point at the data that refute the claim")]
    MissingRefutingData,
}

/// Принятое опровержение: названное основание ПЛЮС ссылка на опровергающие данные.
/// Оба поля сохраняются: выброшенная ссылка оставила бы опровержение, которое
/// нечем сверить с данными, и оно снова стало бы неотличимо от отказа.
///
/// Это `TaskStatus`-уровень: `status()` возвращает именно `Refuted`, а не
/// `Failed` — провал прогона и человеческое опровержение разные факты. Вид записи
/// `EvidenceKind::Refuted` (ADR-0024) — другой слой: опровержение как звено
/// цепочки, а не как статус задачи; совпадение имён намеренно.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refutation {
    basis: String,
    refuting_data: String,
}

/// Собрать опровержение. Основание и ссылка на опровергающие данные обязаны быть
/// непустыми после `trim`: пустая строка и пробелы — это отсутствие значения, а
/// не основание (закон 6).
pub fn refutation(basis: &str, refuting_data: &str) -> Result<Refutation, RefutationError> {
    let basis = basis.trim();
    if basis.is_empty() {
        return Err(RefutationError::MissingBasis);
    }
    let refuting_data = refuting_data.trim();
    if refuting_data.is_empty() {
        return Err(RefutationError::MissingRefutingData);
    }
    Ok(Refutation {
        basis: basis.to_owned(),
        refuting_data: refuting_data.to_owned(),
    })
}

impl Refutation {
    /// Статус, в который переводит принятое опровержение. `Refuted` терминален и
    /// отличен от `Failed`: опровергается предъявленное доказательство, а не
    /// прогон.
    pub fn status(&self) -> TaskStatus {
        TaskStatus::Refuted
    }

    /// Названное основание. Сохраняется целиком: основание — это то, по чему
    /// опровержение можно проверить, а не пометка «опровергнуто».
    pub fn basis(&self) -> &str {
        &self.basis
    }

    /// Ссылка на опровергающие данные. Не теряется: без неё опровержение нечем
    /// сверить с данными.
    pub fn refuting_data(&self) -> &str {
        &self.refuting_data
    }
}

/// Запись `Refuted` — опровержение как звено цепочки (ADR-0024). Основание
/// кладётся в `command`: отдельного поля «почему» у `EvidenceEntry` нет, а
/// основание — это то, по чему опровержение можно проверить. `result_hash` —
/// SHA-256 ССЫЛКИ на опровергающие данные, а не текста основания: сверяют данные,
/// а основание лишь называет причину.
///
/// Четыре binding-поля остаются `None`: человеческое опровержение не является
/// аттестацией запуска, поэтому у него нет ни рецепта, ни политики, ни отпечатка
/// модели, ни коммита. `None` здесь — явное «неприменимо», а не пропуск (закон 6).
///
/// Конструктор authority НЕ является (решение 5): он не решает, было ли
/// опровержение обоснованным, — это делает человек, и отвечает за это он же.
/// Здесь фиксируется форма факта, а не право объявить результат опровергнутым.
pub fn refutation_entry(
    task_id: &str,
    id: &str,
    basis: &str,
    refuting_data: &str,
    ts: &str,
) -> Result<EvidenceEntry, RefutationError> {
    let refutation = refutation(basis, refuting_data)?;
    Ok(EvidenceEntry {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        kind: EvidenceKind::Refuted,
        command: refutation.basis().to_owned(),
        result_hash: sha256_hex(refutation.refuting_data().as_bytes()),
        ts: ts.to_owned(),
        recipe_digest: None,
        policy_digest: None,
        model_fingerprint: None,
        commit_sha: None,
    })
}

/// Состояние результата по отношению к решению человека (решение 5).
///
/// Отдельный тип, а не `bool` и не пустая строка: отсутствие решения и сознательный
/// ответ человека неотличимы, если свести их к флагу (закон 6). `Done` здесь не
/// объявляется: пока человеческой `Validation` нет, результат ЖДЁТ человека, и ни
/// `Done`, ни `gate PASS` не выводятся.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStatus {
    /// `ToolAttestation` в цепочке нет: предъявлять нечего — текст модели
    /// доказательством не является (закон 1). Это НЕ «ждёт человека»: ждать нечего.
    Unattested,
    /// Удостоверение фактической проверки есть, решения человека нет. Результат
    /// ждёт человека; `Done` не объявляется.
    AwaitingHumanValidation,
    /// Человек явно принял результат (человеческая `Validation`, не фикстура).
    HumanValidated,
    /// Человек опроверг доказательства: это решение о предъявленном, а не
    /// отсутствие удостоверения и не ожидание решения. Слить `Refuted` с
    /// `Unattested` или `AwaitingHumanValidation` значило бы сделать «никто не
    /// удостоверял» неотличимым от «кто-то опроверг» (закон 6).
    Refuted,
}

impl EvidenceStatus {
    /// Метка состояния для вывода. Своя строка на каждое: слитое «не готово»
    /// неотличимо от неразобранного, а `gate=ждёт человека` в выводе N1 опирается
    /// именно на неё.
    pub fn label(self) -> &'static str {
        match self {
            EvidenceStatus::Unattested => "нет удостоверения",
            EvidenceStatus::AwaitingHumanValidation => "ждёт человека",
            EvidenceStatus::HumanValidated => "принято человеком",
            EvidenceStatus::Refuted => "опровергнуто",
        }
    }
}

/// Состояние цепочки по решению 5. `evaluate` не трогается: этот разбор отдельный —
/// он про решение человека, а не про вердикт прогона, и пересчёт вердикта здесь
/// выбрал бы сторону в открытом споре (F-2026-09-29-20).
///
/// Человеческая `Validation` — только та, чей `subject` не равен метке фикстуры.
///
/// `Refuted` проверяется ПЕРВЫМ: опровергнутая цепочка остаётся `Refuted`, даже
/// если в ней есть удостоверение или человеческая `Validation`, — принятие
/// опровергнутого недействительно, а пустая цепочка без удостоверения не должна
/// выглядеть как «нечего предъявлять».
pub fn evidence_status(chain: &[EvidenceEntry]) -> EvidenceStatus {
    if chain
        .iter()
        .any(|entry| entry.kind == EvidenceKind::Refuted)
    {
        return EvidenceStatus::Refuted;
    }
    if !chain
        .iter()
        .any(|entry| entry.kind == EvidenceKind::ToolAttestation)
    {
        return EvidenceStatus::Unattested;
    }
    if chain.iter().any(is_human_validation) {
        EvidenceStatus::HumanValidated
    } else {
        EvidenceStatus::AwaitingHumanValidation
    }
}

/// Человеческая ли это `Validation`. Решение обязано быть ЯВНЫМ: запись без
/// `task_id` или с пустым (в том числе пробельным) `subject` — это не решение, а
/// его отсутствие, и продвигать статус она не может (закон 6). Фикстура помечена
/// `FIXTURE_VALIDATION_LABEL` и решением человека не является: иначе тест выдавал
/// бы себя за приёмку (решение 5).
fn is_human_validation(entry: &EvidenceEntry) -> bool {
    entry.kind == EvidenceKind::Validation
        && !entry.task_id.trim().is_empty()
        && !entry.command.trim().is_empty()
        && entry.command != FIXTURE_VALIDATION_LABEL
}

/// Первый индекс записи нужного вида, `None` — вида нет. Отдельный `Option`, а не
/// `0`: отсутствующее звено не должно выглядеть как звено в начале цепочки.
fn first_index(chain: &[EvidenceEntry], kind: EvidenceKind) -> Option<usize> {
    chain.iter().position(|entry| entry.kind == kind)
}

/// Решение этапного gate'а: прогон плюс цепочка, из которой оно выводится.
///
/// `RunOutcome.verdict` читается КАК ДАННОЕ и не пересчитывается: раннер и
/// `validity_gate.py` расходились в предикате остановки (F-2026-09-29-20), и
/// пересчёт внутри гейта молча выбрал бы сторону в открытом споре.
pub fn evaluate(outcome: &RunOutcome, chain: &[EvidenceEntry]) -> GateResult {
    let mut missing: Vec<String> = Vec::new();
    // Опровержение — явное отрицательное решение, а не отсутствие аттестации:
    // отчитаться о нём как о «нет ToolAttestation» значило бы выдать «кто-то
    // опроверг» за «никто не ответил» (закон 6). Возврат идёт ДО проверок
    // удостоверения/валидации/порядка/целостности, чтобы опровергнутая цепочка
    // не стала ни pass, ни «ждёт человека» по умолчанию.
    if let Some(entry) = chain
        .iter()
        .find(|entry| entry.kind == EvidenceKind::Refuted)
    {
        missing.push(format!("refuted: {}", entry.command));
        return GateResult {
            passed: false,
            missing,
            verified: chain.len() as u64,
            chain_tail_hash: chain_tail(chain),
        };
    }
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
    // Порядок проверяется только для тех звеньев, которые ЕСТЬ: отсутствующее
    // звено уже названо выше, и второй причины за него быть не должно.
    let claim_at = first_index(chain, EvidenceKind::ModelClaim);
    let tool_at = first_index(chain, EvidenceKind::ToolAttestation);
    let validation_at = first_index(chain, EvidenceKind::Validation);
    let out_of_order = match (claim_at, tool_at, validation_at) {
        (Some(claim), Some(tool), Some(validation)) => claim >= tool || tool >= validation,
        (None, Some(tool), Some(validation)) => tool >= validation,
        _ => false,
    };
    if out_of_order {
        missing.push(
            "порядок цепочки нарушен: ожидается ModelClaim → ToolAttestation → Validation"
                .to_owned(),
        );
    }
    // Каждое ПРИСУТСТВУЮЩЕЕ звено обязано нести digest: пустой `result_hash` —
    // это не «неизвестный результат», а звено, которое ничего не удостоверяет.
    for entry in chain {
        if entry.result_hash.is_empty() {
            missing.push(format!("запись {} без digest: result_hash пуст", entry.id));
        }
        if entry.command.is_empty() {
            match entry.kind {
                EvidenceKind::ToolAttestation => missing
                    .push("ToolAttestation без команды: записанный запуск не назван".to_owned()),
                EvidenceKind::Validation => {
                    missing.push("Validation без субъекта: решение не названо".to_owned())
                }
                EvidenceKind::ModelClaim => {}
                // Refuted сюда не доходит: опровержение возвращает отказ выше.
                // Ветка пуста только ради исчерпывающего match.
                EvidenceKind::Refuted => {}
            }
        }
        // Привязка — часть доказательства, а не украшение: удостоверение без
        // рецепта, политики или коммита проверять не по чему. Для остальных видов
        // `None` остаётся явным «неприменимо» (закон 6).
        if entry.kind == EvidenceKind::ToolAttestation {
            if entry.recipe_digest.is_none() {
                missing.push(
                    "ToolAttestation без recipe_digest: привязка к рецепту не названа".to_owned(),
                );
            }
            if entry.policy_digest.is_none() {
                missing.push(
                    "ToolAttestation без policy_digest: привязка к политике не названа".to_owned(),
                );
            }
            if entry.commit_sha.is_none() {
                missing.push(
                    "ToolAttestation без commit_sha: привязка к коммиту не названа".to_owned(),
                );
            }
        }
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
        hash_optional_text(
            &mut hasher,
            entry.recipe_digest.as_ref().map(|digest| digest.0.as_str()),
        );
        hash_optional_text(
            &mut hasher,
            entry.policy_digest.as_ref().map(|digest| digest.0.as_str()),
        );
        hash_optional_text(
            &mut hasher,
            entry
                .model_fingerprint
                .as_ref()
                .map(|fingerprint| fingerprint.0.as_str()),
        );
        hash_optional_text(
            &mut hasher,
            entry.commit_sha.as_ref().map(|sha| sha.0.as_str()),
        );
        tail = format!("{:x}", hasher.finalize());
    }
    tail
}

/// Хеш необязательной привязки: маркер наличия, затем длина, затем значение.
/// `None` (маркер 0) и `Some("")` (маркер 1, длина 0) обязаны различаться:
/// иначе отсутствие привязки и пустая привязка сошлись бы в один хвост.
fn hash_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(text) => {
            hasher.update([1u8]);
            hasher.update((text.len() as u64).to_be_bytes());
            hasher.update(text.as_bytes());
        }
        None => {
            hasher.update([0u8]);
            hasher.update(0u64.to_be_bytes());
        }
    }
}

/// Имя вида записи в хвосте — те же строки, что у PolyForge и в сериализации
/// контракта: своё имя разошлось бы с контрактом, и записи разных видов сошлись бы
/// в одну.
fn kind_name(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::ModelClaim => "ModelClaim",
        EvidenceKind::ToolAttestation => "ToolAttestation",
        EvidenceKind::Validation => "Validation",
        EvidenceKind::Refuted => "Refuted",
    }
}

/// SHA-256 в hex нижнего регистра. Крейт из `[workspace.dependencies]`, а не
/// `DefaultHasher`: тот меняется между версиями rustc и даёт 64 бита — в цепочке
/// доказательств это уверенный вид подмены, которой не было.
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
