//! Approval — «кого спросить и куда положить вопрос». Раннер решает «можно ли», и это
//! другой вопрос; общий файл означал бы, что пакет 05 и этот не могут идти параллельно.
//!
//! Ключевое требование В4: «запрещено политикой» и «никто не ответил» обязаны остаться
//! разными **и в статусе задачи**, и в потоке. Отсюда `StatusEffect` вместо простого
//! `TaskStatus` — см. раздел «Изменение сигнатуры `next_status`» в пакете.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::runner::CallVerdict;
use serde::Serialize;
use stellwerk_core::contract::{PolicyLevel, RiskClass, TaskStatus};
use stellwerk_gateway::permission::ToolRisk;

pub const INBOX_ENV: &str = "STELLWERK_INBOX";
pub const DEFAULT_INBOX: &str = ".stellwerk/inbox";

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    /// Переменная задана, но пуста. Это **не** то же, что переменная не задана: пустая
    /// строка почти всегда опечатка, и молчаливый откат на путь по умолчанию отправил бы
    /// вопросы в рабочий каталог.
    #[error("`{INBOX_ENV}` задан пустым: это опечатка, а не отсутствие переменной")]
    EmptyInboxEnv,
    #[error("не удалось работать с inbox: {0}")]
    Io(#[from] std::io::Error),
    /// У вопроса нет одного из обязательных полей. `String::new()` молча стал бы пустым
    /// значением, а пустой `run_id` дал бы файл, который нельзя связать с прогоном.
    #[error("вопрос неполон: {0}")]
    IncompleteRequest(&'static str),
}

/// Что вердикт делает со статусом задачи. Именованный тип, а не `Option`: `None` в этом
/// проекте трижды оказывалось ловушкой (F-10, F-30, `unwrap_or(R0)`), потому что
/// отсутствие читается как «неизвестно», а здесь это сознательное «оставить как есть».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEffect {
    /// Вердикт не меняет статус; вызывающий в БД не пишет.
    Keep,
    /// Вердикт требует перехода; вызывающий сверяется с `can_transition_to` и пишет.
    Transition(TaskStatus),
}

/// Вопрос, ждущий человека. Сериализуется при `park`, и риск уходит на провод строкой:
/// `ToolRisk` не выводит `Serialize`, а менять `permission.rs` пакет не может.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub run_id: String,
    pub task_id: String,
    pub tool: String,
    /// Дайджест пустого тела, а не само тело: у `CallRequest` полей с аргументами ноль,
    /// и пустая строка означала бы «не посчитали» (закон 6).
    pub empty_args_digest: String,
    pub risk: ToolRisk,
    /// `L5Owner`: ответить обязан человек с высшими полномочиями.
    pub answerable_by: PolicyLevel,
    pub window_hours: u32,
    /// RFC 3339 UTC.
    pub expires_at: String,
}

/// Окно ответа: `R4` — 4ч, всё остальное — 24ч. Правило взято из `session.rs`
/// (`answer_window_secs`), чтобы окно не расходилось между двумя местами.
fn window_hours(risk: ToolRisk) -> u32 {
    match risk {
        ToolRisk::Classified(RiskClass::R4) => 4,
        ToolRisk::Classified(_) | ToolRisk::Unclassified => 24,
    }
}

/// Метка риска для JSON. Не `Debug`: на проводе должен лежать читаемый класс, иначе
/// расследование читает `Classified(R3)` вместо `R3`.
fn risk_label(risk: ToolRisk) -> String {
    match risk {
        // У `RiskClass` нет serde-переименования, поэтому имя варианта и есть метка на
        // проводе. Через serde, а не через `Debug`: форма сети принадлежит типу из
        // контракта, а не его отладочной распечатке.
        ToolRisk::Classified(class) => serde_json::to_string(&class)
            .unwrap_or_else(|_| format!("{class:?}"))
            .trim_matches('"')
            .to_owned(),
        ToolRisk::Unclassified => "unclassified".to_owned(),
    }
}

pub fn request(run_id: &str, task_id: &str, tool: &str, risk: ToolRisk) -> ApprovalRequest {
    let window = window_hours(risk);
    let expires_at = rfc3339_utc(unix_secs().saturating_add(i64::from(window) * 3_600));
    ApprovalRequest {
        run_id: run_id.to_owned(),
        task_id: task_id.to_owned(),
        tool: tool.to_owned(),
        empty_args_digest: sha256_hex(b""),
        risk,
        answerable_by: PolicyLevel::L5Owner,
        window_hours: window,
        expires_at,
    }
}

/// Маппинг вердикта в эффект. Про **текущий** статус не знает и знать не должна: единственный
/// источник — `status_of` у вызывающего (F-13), поэтому его нет и в сигнатуре.
pub fn next_status(verdict: &CallVerdict, risk: ToolRisk) -> StatusEffect {
    match verdict {
        // R5 необратимо: ждать человека по вопросу, который человек всё равно не сможет
        // разрешить, — ловушка. `Unclassified` — вопрос задать некому.
        CallVerdict::Ask | CallVerdict::Suspended => {
            if matches!(
                risk,
                ToolRisk::Classified(RiskClass::R5) | ToolRisk::Unclassified
            ) {
                StatusEffect::Transition(TaskStatus::Failed)
            } else {
                StatusEffect::Transition(TaskStatus::Suspended)
            }
        }
        CallVerdict::Deny { .. } => StatusEffect::Transition(TaskStatus::Failed),
        CallVerdict::Allow => StatusEffect::Keep,
    }
}

/// Каталог вопросов. Отсутствие значения и его пустое значение — разные случаи, и
/// разница обязана сохраняться: пустая строка почти всегда опечатка, а молчаливый
/// откат на путь по умолчанию отправил бы вопросы в рабочий каталог.
///
/// Глобальное окружение функция не читает: значение передаёт вызывающий (точка
/// входа приложения). Чтение `std::env` из библиотеки сделало бы поведение
/// зависящим от общего состояния процесса, а это источник гонок.
pub fn inbox_dir(configured: Option<&str>) -> Result<PathBuf, ApprovalError> {
    match configured {
        Some(value) if value.trim().is_empty() => Err(ApprovalError::EmptyInboxEnv),
        Some(value) => Ok(PathBuf::from(value)),
        None => Ok(PathBuf::from(DEFAULT_INBOX)),
    }
}

#[derive(Serialize)]
struct Parked<'a> {
    run_id: &'a str,
    task_id: &'a str,
    tool: &'a str,
    empty_args_digest: &'a str,
    risk: String,
    answerable_by: PolicyLevel,
    window_hours: u32,
    expires_at: &'a str,
}

/// Кладёт вопрос в `<inbox>/<run_id>.json`. Имя файла — из `run_id`, поэтому два вопроса
/// одного прогона различаются, а вопросы разных прогонов не делят файл.
pub fn park(request: &ApprovalRequest, inbox: &Path) -> Result<(), ApprovalError> {
    for (name, value) in [
        ("run_id", &request.run_id),
        ("task_id", &request.task_id),
        ("tool", &request.tool),
        ("empty_args_digest", &request.empty_args_digest),
        ("expires_at", &request.expires_at),
    ] {
        if value.trim().is_empty() {
            return Err(ApprovalError::IncompleteRequest(name));
        }
    }
    std::fs::create_dir_all(inbox)?;
    let body = Parked {
        run_id: &request.run_id,
        task_id: &request.task_id,
        tool: &request.tool,
        empty_args_digest: &request.empty_args_digest,
        risk: risk_label(request.risk),
        answerable_by: request.answerable_by,
        window_hours: request.window_hours,
        expires_at: &request.expires_at,
    };
    let target = inbox.join(format!("{}.json", request.run_id));
    std::fs::write(
        target,
        serde_json::to_vec_pretty(&body).map_err(std::io::Error::other)?,
    )?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

fn unix_secs() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    i64::try_from(secs).unwrap_or(i64::MAX)
}

/// RFC 3339 UTC в виде `YYYY-MM-DDTHH:MM:SSZ`. Свой форматтер, а не зависимость: `cargo add`
/// пакетом запрещён, а в проекте уже есть такой же (`session.rs`).
fn rfc3339_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil_from_days, алгоритм Хиннанта: инверсия days_from_civil без деления на векторы.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!(
        "{year:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}
