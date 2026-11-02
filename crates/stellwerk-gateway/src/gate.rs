//! Итоговое решение по вызову: риск инструмента, полномочия вызывающего и класс
//! данных сводятся в один `Verdict`.
//!
//! Три независимых правила, порядок обязателен: риск инструмента, полномочия
//! вызывающего, класс данных. `Deny` по любому из них возвращается немедленно;
//! иначе `Ask` при R2/R3/R4, иначе `Allow`.
//!
//! Правила принимаются ТОЛЬКО через предикаты контракта и `permission_for`:
//! своей таблицы рисков и своих уровней здесь нет.

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::permission::{permission_for, Permission, ToolRisk};
use stellwerk_core::contract::{Action, DataClass, ModelBinding, PolicyLevel};

/// Запрос на вызов инструмента: всё, о чём gateway знает до решения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRequest {
    /// Имя инструмента: в `Ask` человек читает его.
    pub tool: String,
    /// MCP-модуль, из которого инструмент.
    pub module: String,
    /// Риск инструмента. `Unclassified` — отдельное состояние, а не нулевой риск.
    pub risk: ToolRisk,
    /// Уровень полномочий вызывающего.
    pub caller: PolicyLevel,
    /// Класс чувствительности данных, уходящих по этому вызову.
    pub data: DataClass,
    /// Binding модели, которая их обработает.
    pub model: ModelBinding,
    /// Есть ли кому задать вопрос. `false` — неинтерактивный прогон.
    pub interactive: bool,
}

/// Инструмент меняет содержимое файла. Неизвестное имя само по себе пишущим не
/// считается: список назван явно, а не выведен из подстроки имени.
pub fn writes_content(tool: &str) -> bool {
    matches!(tool, "write" | "edit" | "write_note")
}

/// SHA-256 содержимого. Считается только по фактическим байтам.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentDigest(String);

/// Заявленный digest не является 64 шестнадцатеричными символами: это отсутствие
/// данных о содержимом, а не «хеш нуля».
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DigestError {
    #[error("content digest `{value}` is not 64 hex characters")]
    Malformed { value: String },
}

impl ContentDigest {
    /// SHA-256 над фактическими байтами содержимого, не над текстом модели.
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    /// Разбор заявленного digest с проверкой формы: мусор — отдельный отказ.
    pub fn parse(hex: &str) -> Result<Self, DigestError> {
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DigestError::Malformed {
                value: hex.to_owned(),
            });
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

/// Полный аргумент вызова вместе с digest содержимого.
///
/// Поля закрыты: запрос собирается только [`RequestedCall::new`], который
/// пересчитывает digest по переданным байтам и сверяет с заявленным. Поэтому
/// заявленный digest не может разойтись с содержимым незаметно.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedCall {
    tool: String,
    path: Option<String>,
    argument: Vec<u8>,
    content_digest: ContentDigest,
}

/// Запрос не собирается: заявленный digest не описывает переданные байты.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RequestRefusal {
    #[error("claimed content digest `{claimed}` does not match the bytes: actual `{actual}`")]
    DigestMismatch { claimed: String, actual: String },
}

impl RequestedCall {
    /// Собирает запрос из ПОЛНОГО аргумента и заявленного digest. Digest
    /// пересчитывается по байтам; расхождение — отказ, а не доверие заявителю.
    pub fn new(
        tool: impl Into<String>,
        path: Option<String>,
        argument: Vec<u8>,
        claimed: &ContentDigest,
    ) -> Result<Self, RequestRefusal> {
        let actual = ContentDigest::of(&argument);
        if actual != *claimed {
            return Err(RequestRefusal::DigestMismatch {
                claimed: claimed.as_hex().to_owned(),
                actual: actual.as_hex().to_owned(),
            });
        }
        Ok(Self {
            tool: tool.into(),
            path,
            argument,
            content_digest: actual,
        })
    }

    pub fn tool(&self) -> &str {
        &self.tool
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn argument(&self) -> &[u8] {
        &self.argument
    }

    pub fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }
}

/// Авторизованный вызов: существует только после решения гейта и побайтово
/// связан с digest содержимого.
///
/// Поля закрыты, а конструктор [`AuthorizedCall::issue`] не является `pub`:
/// собрать значение вне этого модуля нельзя. Так право выдать разрешение
/// остаётся у гейта, а не у вызывающего кода.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedCall {
    tool: String,
    path: String,
    argument: Vec<u8>,
    content_digest: ContentDigest,
    scope: String,
}

/// Текущие байты разошлись с авторизованными: содержимое подменили после решения.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("authorized content `{authorized}` does not match current bytes `{actual}`")]
pub struct ContentMismatch {
    pub authorized: String,
    pub actual: String,
}

impl AuthorizedCall {
    /// Единственная дверь к значению: только она и только внутри модуля.
    fn issue(requested: &RequestedCall, basis_scope: &str) -> Option<Self> {
        let scope = format!("{basis_scope}#sha256:{}", requested.content_digest.as_hex());
        Some(Self {
            tool: requested.tool.clone(),
            path: requested.path.clone()?,
            argument: requested.argument.clone(),
            content_digest: requested.content_digest.clone(),
            scope,
        })
    }

    pub fn tool(&self) -> &str {
        &self.tool
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn argument(&self) -> &[u8] {
        &self.argument
    }

    pub fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }

    /// Основание разрешения вместе с digest: разрешение разного содержимого
    /// не выглядит одинаково.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Сверяет текущие байты с авторизованными. Подмена после авторизации — отказ.
    pub fn verify_content(&self, current: &[u8]) -> Result<(), ContentMismatch> {
        let actual = ContentDigest::of(current);
        if actual == self.content_digest {
            return Ok(());
        }
        Err(ContentMismatch {
            authorized: self.content_digest.as_hex().to_owned(),
            actual: actual.as_hex().to_owned(),
        })
    }
}

/// Решение по типизированному вызову. `Refused` несёт причину, потому что без неё
/// агент не поймёт, что именно не совпало.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallDecision {
    Authorized(AuthorizedCall),
    Refused { reason: String },
}

/// Авторизует запись по полному аргументу и digest. Содержимое уже сверено в
/// [`RequestedCall::new`], а `scope` получает digest, поэтому два разных тела под
/// одним путём не дают одно и то же разрешение.
pub fn authorize_write(requested: &RequestedCall, basis_scope: &str) -> CallDecision {
    AuthorizedCall::issue(requested, basis_scope).map_or_else(
        || CallDecision::Refused {
            reason: "write_without_path: запись без пути не к чему применить".to_owned(),
        },
        CallDecision::Authorized,
    )
}

/// Решение по вызову.
///
/// `Suspended` отделён от `Deny` намеренно: «запрещено политикой» и «никто не
/// ответил» — разные ситуации (В4), и в статусе они должны различаться.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Разрешено.
    Allow,
    /// Спросить человека; без интерактива превращается в `Suspended`.
    Ask,
    /// Вопрос задан, ответа нет.
    Suspended,
    /// Отказ. Причина обязательна: без неё агент не поймёт, что делать.
    Deny { reason: String },
}

/// Итоговый вердикт по вызову.
///
/// Окно ответа — 24ч, для R4 — 4ч, и его истечение даёт `Failed(approval_timeout)`,
/// а не `Deny` (В4). Отсчёт живёт в пакете inbox/approval: у `decide` нет часов,
/// а выдуманное время решило бы судьбу прогона, не спросив никого.
pub fn decide(req: &CallRequest) -> Verdict {
    // Правило 1 — риск инструмента, через `permission_for`: своей таблицы нет.
    let permission = match permission_for(req.risk) {
        Ok(permission) => permission,
        Err(err) => return deny(format!("risk_table_incomplete: {err}")),
    };

    let needs_approval = match req.risk {
        // В2: `Unclassified` — отсутствие данных, а не нулевой риск. Спросить про
        // неизвестный инструмент вправе только владелец политики, и лишь когда
        // спросить реально: без интерактива «пока подумаем» неотличимо от
        // разрешения, а `Suspended` неуместен — вопрос задать некому.
        ToolRisk::Unclassified => {
            if !req.interactive {
                return deny(
                    "unclassified_tool: риск инструмента не определён, а спросить некого — классифицируйте тул в modules.toml",
                );
            }
            if !req.caller.permits(Action::GrantRoles) {
                return deny(
                    "unclassified_tool: риск инструмента не определён; ответить про него может только владелец политики",
                );
            }
            true
        }
        ToolRisk::Classified(class) => {
            // R5: необратимое разрушение не делегируется агенту ни при каком
            // уровне, включая L5Owner. Отказ приходит из таблицы до `Ask`, поэтому
            // R5 не спрашивается и не suspend'ится: владелец делает force-push руками.
            if permission == Permission::Deny {
                return deny(format!(
                    "risk_denied: класс {class:?} запрещён таблицей RISK_PERMISSION"
                ));
            }
            // Уровень берётся только из контракта (ADR-0002: R3 и R4 требуют
            // L5Owner). Нехватка уровня отказывает ДО Ask: у L2Contributor нельзя
            // спросить разрешение на сетевой выход, его принимает тот, у кого право есть.
            if req.caller < class.required_level() {
                return deny(format!(
                    "insufficient_level: {:?} ниже требуемого {:?}",
                    req.caller,
                    class.required_level()
                ));
            }
            permission == Permission::Ask
        }
    };

    // Правило 3 — данные, единственным предикатом `permits_model`. Binding
    // обязателен в типе, поэтому «привязки нет» не представимо: уйти безымянно
    // данные не могут.
    if !req.data.permits_model(&req.model) {
        return deny(format!(
            "data_class_exceeds_model: {:?} выше, чем держит binding {:?} ({:?})",
            req.data, req.model.provider, req.model.retention
        ));
    }

    if !needs_approval {
        return Verdict::Allow;
    }
    // В4: `Ask` без интерактива — `Suspended`, а не `Deny`: «никто не ответил» и
    // «запрещено политикой» обязаны различаться в статусе и в evidence.
    if req.interactive {
        Verdict::Ask
    } else {
        Verdict::Suspended
    }
}

/// `Deny` с обязательной причиной: пустая строка не сказала бы агенту, что делать.
fn deny(reason: impl Into<String>) -> Verdict {
    Verdict::Deny {
        reason: reason.into(),
    }
}
