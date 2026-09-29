//! Сессия: точка, где permission, gate, shaping и receipt встречаются в одном проходе.
//!
//! Gateway инструменты не выполняет. `handle` — RED-заглушка M3/packet-11a: маршрутизацию
//! закрепляет 11b, вопрос и сбой — 11c, реализация — packet-12.
//!
//! Шов `ToolBackend` и опрос `drive` объявлены здесь намеренно: у крейта нет `tokio`, а
//! `Cargo.toml` вне `files_allowed`, поэтому 11b и 11c берут их отсюда, а не пишут свои.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use stellwerk_core::contract::PolicyLevel;

/// Реэкспорты, а НЕ вторые объявления: второе сделало бы риск при монтаже и риск в
/// `decide` разными типами, а перевод между ними рано или поздно принёс бы `unwrap_or(R0)`.
pub use crate::gate::{CallRequest, Verdict};
pub use crate::mount::MountPlan;
pub use crate::permission::ToolRisk;
pub use crate::receipt::Receipt;
pub use crate::shaping::Shaped;

/// Сессия: смонтированный набор, полномочия и бюджет одного прогона.
#[derive(Debug, Clone)]
pub struct Session {
    /// Идентификатор прогона: он же имя вопроса в inbox и `run_id` в чеке.
    pub id: String,
    /// Что смонтировано: инструмент вне плана не вызывается.
    pub plan: MountPlan,
    /// Уровень полномочий вызывающего — вход `decide`.
    pub caller: PolicyLevel,
    /// Бюджет контекста на прогон, в токенах.
    pub budget_tokens: u64,
    /// Счётчик состоявшихся вызовов: невидимый вызов не считается.
    pub calls: u64,
    /// Глубина субагентов (В3). Поле, а не текст инструкции: при `risk >= R2` гейт
    /// обязан держать её 0, а правило без наблюдаемого состояния через полгода не
    /// отличить от отсутствия правила.
    pub subagent_depth: u32,
    /// Корень рабочего места: отсюда пишется `.stellwerk/inbox/<run_id>.json`. Без него
    /// вопросу некуда лечь, и «записали» не отличить от «не записали».
    pub root: PathBuf,
}

impl Session {
    /// Провести один вызов: permission → gate → shaping → receipt.
    ///
    /// Отдаёт future, а не `Result`: исполнителя в крейте нет, и `drive` ниже доводит
    /// его сам. RED M3/packet-11a: поведение — M3/packet-12.
    pub fn handle<'a>(
        &'a mut self,
        req: &'a CallRequest,
        backend: &'a dyn ToolBackend,
    ) -> Pin<Box<dyn Future<Output = Result<CallOutcome, SessionError>> + Send + 'a>> {
        let _ = (self, req, backend);
        unimplemented!("M3/packet-12: session::handle (маршрутизация 11b, вопрос 11c)")
    }
}

/// Итог вызова, дошедший до вызывающей стороны.
#[derive(Debug)]
pub struct CallOutcome {
    /// Решение. `Deny` и `Suspended` — успешный проход, а не сбой сессии.
    pub verdict: Verdict,
    /// Усечение вывода, если вызов состоялся и вывод не поместился в порог.
    pub shaped: Option<Shaped>,
    /// Чек вызова. `None` — «вызова не было»; у состоявшегося он обязателен.
    pub receipt: Option<Receipt>,
}

/// Вопрос оператору: ложится в inbox и читается обратно человеком или следующим
/// прогоном, поэтому сериализуем.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Прогон, задавший вопрос: он же имя файла в inbox.
    pub run_id: String,
    /// Инструмент, о котором спрашивают: человек читает его, а не хеш.
    pub tool: String,
    /// Риск инструмента. `Unclassified` — состояние, а не нулевой риск.
    #[serde(with = "risk_wire")]
    pub risk: ToolRisk,
    /// SHA-256 тела аргументов: подтверждаются именно эти аргументы, и хеш обязан
    /// пережить круг через inbox без искажения.
    pub args_hash: String,
    /// Когда спросили, RFC 3339 UTC.
    pub requested_at: String,
    /// До какого ждут ответа, RFC 3339 UTC. Истечение — `ApprovalTimeout`, не `Deny`.
    pub expires_at: String,
}

/// Провод для `ToolRisk`: класс сериализуется СВОИМ impl'ом контракта (`"R1"`), а
/// `Unclassified` — отдельным словом. Перечисления строк риска здесь нет: оно стало бы
/// вторым источником истины о классах, которые уже описаны в контракте.
mod risk_wire {
    use super::ToolRisk;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use stellwerk_core::contract::RiskClass;

    /// Слово НЕИЗВЕСТНОГО риска на проводе: не `"R0"`, потому что отсутствие данных
    /// обязано пережить круг, а не стать нулевым риском.
    const UNCLASSIFIED: &str = "unclassified";

    pub fn serialize<S: Serializer>(risk: &ToolRisk, out: S) -> Result<S::Ok, S::Error> {
        match risk {
            ToolRisk::Classified(class) => class.serialize(out),
            ToolRisk::Unclassified => out.serialize_str(UNCLASSIFIED),
        }
    }

    /// Непрочитанная строка — ошибка разбора, а НЕ `R0`: класс угадывать нельзя.
    pub fn deserialize<'de, D: Deserializer<'de>>(inbox: D) -> Result<ToolRisk, D::Error> {
        let raw = String::deserialize(inbox)?;
        if raw == UNCLASSIFIED {
            return Ok(ToolRisk::Unclassified);
        }
        // `Value`, а не `from_str`: строка на проводе уже разобрана, а `R2` без кавычек
        // JSON не валиден.
        serde_json::from_value::<RiskClass>(serde_json::Value::String(raw))
            .map(ToolRisk::Classified)
            .map_err(serde::de::Error::custom)
    }
}

/// Шов вызова инструмента — один метод. Живой процесс в тесте не нужен: gateway
/// инструменты не выполняет, а подделка подставляется вместо него.
pub trait ToolBackend {
    /// Вызвать инструмент и отдать его вывод. `Err` — инструмент упал: сбой чужого
    /// клиента, а не отказ политики.
    fn invoke(
        &self,
        tool: &str,
        input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>>;
}

/// Сбой чужого инструмента. Newtype над чужим текстом, а НЕ перечень причин: опись того,
/// как чужой клиент падает, стала бы вторым источником истины о нём самом.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct ToolError(pub String);

/// Отказ сессии. Причина обязана называться, а разные отказы обязаны быть разными
/// вариантами: «запрещено политикой» и «никто не ответил» сливать нельзя (В4).
#[derive(Debug, Error)]
pub enum SessionError {
    /// Инструмент упал, и чек едет ВНУТРИ ошибки: при `Result<CallOutcome, _>` он иначе
    /// исчез бы, а вызов без следа неотличим от вызова, которого не было.
    #[error("tool: {source}")]
    Tool {
        /// Чужой текст сбоя — дословно, без разбора причин.
        source: ToolError,
        /// Чек состоявшегося вызова: сбой — тоже факт. В коробке — чек на 208 байт против
        /// пустого `ApprovalTimeout`, и clippy справедливо требует индирекции.
        receipt: Box<Receipt>,
    },
    /// Окно ответа истекло. Отдельный вариант, а НЕ `Deny`: на вопрос не ответили, но
    /// никто не отказывал, и в расследовании это разные вещи.
    #[error("approval_timeout: окно ответа истекло")]
    ApprovalTimeout,
}

/// Довести future до `Ready` без чужого исполнителя: крейт от `tokio` не зависит, а
/// `Cargo.toml` вне `files_allowed`. Один опрос на 11a, 11b и 11c, а не свой в каждом.
pub fn drive<F: Future>(future: F) -> F::Output {
    /// Будильщик молчит: с готовым backend будущее не ждёт, и ждать тут нечего.
    struct Park;
    impl Wake for Park {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Park));
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
