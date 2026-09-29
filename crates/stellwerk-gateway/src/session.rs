//! Сессия: точка, где permission, gate, shaping и receipt встречаются в одном проходе.
//!
//! Gateway инструменты не выполняет: `handle` спрашивает шов `ToolBackend`. Линейный проход
//! (монтаж, решение, вызов, чек) — M3/packet-12a; вопрос и сбой — 12b, усечение — 12c
//! (F-24: калибровки `tokens_per_byte` у слоя нет, а усечение без неё — выдуманный byte price).
//!
//! Шов `ToolBackend` и опрос `drive` объявлены здесь намеренно: у крейта нет `tokio`, а
//! `Cargo.toml` вне `files_allowed`, поэтому 11b и 11c берут их отсюда, а не пишут свои.

use std::fs;
use std::future::{ready, Future};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use stellwerk_core::contract::{PolicyLevel, RiskClass};

/// Реэкспорты, а НЕ вторые объявления: второе сделало бы риск при монтаже и риск в
/// `decide` разными типами, а перевод между ними рано или поздно принёс бы `unwrap_or(R0)`.
pub use crate::gate::{CallRequest, Verdict};
pub use crate::mount::MountPlan;
pub use crate::permission::ToolRisk;
pub use crate::receipt::Receipt;
pub use crate::shaping::Shaped;

use crate::gate::decide;

/// Измеренная цена байта вывода: данных у слоя нет, они приходят параметром от загрузчика,
/// который читает `modules.toml` снаружи. Хардкод здесь был бы вторым источником истины.
#[derive(Debug, Clone)]
pub struct TokenCalibration {
    /// Провайдер замера: один `model_id` у разных провайдеров токенизуется по-разному,
    /// а значит и стоит по-разному.
    pub provider: String,
    /// Модель, к которой замер относится: калибровка чужой модели усечёт по чужому
    /// токенизатору, и отличить это от своей нечем.
    pub model_id: String,
    /// Токенов в байте. Ноль означал бы «вывод ничего не стоит», а «никто не мерил» —
    /// другое состояние, поэтому отсутствие выражается `None`, а не `0.0` (закон 6).
    pub tokens_per_byte: f64,
    /// Дата замера, RFC 3339 UTC: устаревшая калибровка обязана быть видна датой, а не
    /// тем, что её кто-то применил.
    pub measured_at: String,
    /// Происхождение замера: правка плана и снятие с живой модели стареют по-разному,
    /// а подмена одного другим неразличима.
    pub source: String,
}

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
    /// Калибровка этой модели: `None` — замера нет, а не «вывод бесплатный». Усечение
    /// вывода по бюджету читает это поле (12c), а не байты наугад.
    pub token_calibration: Option<TokenCalibration>,
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
    /// Провести один вызов: монтаж → `decide` → вызов → чек.
    ///
    /// Отдаёт future, а не `Result`: исполнителя в крейте нет, и `drive` ниже доводит
    /// его сам. Вопрос оператору и сбой инструмента — M3/packet-12b, усечение вывода,
    /// которому нужна калибровка `tokens_per_byte`, — 12c.
    pub fn handle<'a>(
        &'a mut self,
        req: &'a CallRequest,
        backend: &'a dyn ToolBackend,
    ) -> Pin<Box<dyn Future<Output = Result<CallOutcome, SessionError>> + Send + 'a>> {
        match self.refusal(req) {
            Ok(Some(verdict)) => {
                // Отказ — тоже чек: вызов без следа неотличим от небывшего (REVIEW §3.1).
                // Тела у него нет, поэтому хеш пустого вывода, а не «не посчитали».
                let receipt = self.receipt(req, &verdict, &sha256_hex(&[]), 0);
                return Box::pin(ready(Ok(CallOutcome {
                    verdict,
                    shaped: None,
                    receipt,
                })));
            }
            // Истёкшее окно — сбой сессии, а не отказ: инструмент не звали, а чек в чеке
            // приостановки записал бы решение, которого не принимали (В4).
            Err(err) => return Box::pin(ready(Err(err))),
            Ok(None) => {}
        }
        // Отказа нет — остаётся единственная оставшаяся ветка `decide`.
        let verdict = Verdict::Allow;
        // Аргументов у `CallRequest` нет, и вход вызова — пустое тело: подставить
        // отсебятину значило бы передать инструменту не то, о чём потом скажет чек.
        let input = String::new();
        let started = Instant::now();
        // `invoke` вызван здесь, а не внутри future: `Send`-future не удерживает
        // `&dyn ToolBackend` без `Sync`, а побочные эффекты живут в самом future.
        let called = backend.invoke(&req.tool, &input);
        Box::pin(async move {
            let output = match called.await {
                Ok(output) => output,
                // Сбой чужого инструмента. Порядок обязателен: сначала чек из того,
                // что успело уйти, потом ошибка — при `Result<CallOutcome, _>` след
                // исчез бы, а вызов без следа неотличим от вызова, которого не было.
                // `Box<Receipt>` обязателен: чек на 208 байт против пустого
                // `ApprovalTimeout` — это индирекция, которую справедливо требует clippy.
                Err(source) => {
                    let duration_ms =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let receipt = self.receipt(req, &verdict, &sha256_hex(&[]), duration_ms);
                    // `receipt` — `None` РОВНО при `Unclassified`, а `decide` при
                    // `Unclassified` `Allow` не выдаёт (`gate.rs`: без интерактива —
                    // `Deny`, с интерактивом — `Ask`). Подставить вместо чека `R0`
                    // было бы ровно тем нулевым риском, который запрещает закон 6.
                    let Some(receipt) = receipt else {
                        unreachable!("`decide` не разрешает вызов при неизвестном риске");
                    };
                    // Счётчик не растёт: состоявшийся вызов — тот, чей вывод дошёл; упавший
                    // учтён чеком внутри ошибки, и второго следа ему не нужно.
                    return Err(SessionError::Tool {
                        source,
                        receipt: Box::new(receipt),
                    });
                }
            };
            // Счётчик — после успеха: невидимый вызов не работа.
            self.calls += 1;
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            // Хеш полного тела считаем сами, а не берём из `shape`: калибровки у слоя нет
            // (`modules.toml` недостижим, а поле `Session` добавить нельзя), и 12a не решает,
            // влезает ли вывод в порог. Хеш — факт о теле, а не оценка его цены.
            let receipt = self.receipt(req, &verdict, &sha256_hex(output.as_bytes()), duration_ms);
            // `shaped` — `None`, а не «усечённый вывод»: этот слой не усекает и не решает,
            // что усекать, потому что калибровки у него нет. Поле описано как «усечение
            // вывода», и `None` здесь — честное «не усекал», а не «не посчитали».
            Ok(CallOutcome {
                verdict,
                shaped: None,
                receipt,
            })
        })
    }

    /// Решение, останавливающее вызов: монтаж, а затем `decide`. `None` — вызов
    /// разрешён и уходит в инструмент. `Err` — окно ответа по уже заданному вопросу
    /// истекло, и это сбой сессии, а не отказ (В4).
    fn refusal(&self, req: &CallRequest) -> Result<Option<Verdict>, SessionError> {
        // Монтаж ДО `decide`: «смонтирован ли инструмент» к риску отношения не имеет, а
        // вопрос про инструмент, которого нет, задавать некому.
        if let Some(reason) = mount_refusal(&self.plan, &req.tool) {
            return Ok(Some(Verdict::Deny { reason }));
        }
        let verdict = decide(req);
        match verdict {
            // Решение — только `decide`, своей таблицы рисков у сессии нет. `R5` отсекает
            // он сам (`Permission::Deny` до `Ask`-ветки `RISK_PERMISSION`): второй отсек
            // риска здесь был бы копией таблицы, а отсек шире R5 убил бы `Ask` у R3/R4.
            Verdict::Deny { .. } => Ok(Some(verdict)),
            // Вопрос без подтверждения не разрешение (REVIEW §4.4), а `Suspended` обязан
            // отличаться от `Deny` (В4): уходят в `suspend` вместе с inbox и окном ответа.
            Verdict::Ask | Verdict::Suspended => self.suspend(req),
            Verdict::Allow => Ok(None),
        }
    }

    /// Вопрос оператору: `Suspended` и вопрос на диске, либо `Err`, если окно ответа по
    /// уже заданному вопросу истекло.
    ///
    /// `Ask` (интерактивный прогон) и `Suspended` (неинтерактивный) сходятся в одном
    /// действии: `handle` не умеет получать ответы, а читает их тело прогона (M2-10).
    /// Цикл возобновления здесь был бы вторым источником истины о прогоне.
    ///
    /// Глубина субагентов на этом пути остаётся 0: при R2+ субагент не создаётся, а
    /// `subagent_depth` — наблюдаемое состояние этого правила (В3). Текстовый запрет
    /// модель игнорирует, а правило без поля через полгода не отличить от его отсутствия.
    fn suspend(&self, req: &CallRequest) -> Result<Option<Verdict>, SessionError> {
        let dir = self.root.join(".stellwerk/inbox");
        let path = dir.join(format!("{}.json", self.id));
        // Уже заданный вопрос — источник истины об окне: срок нельзя выводить из «сейчас»,
        // иначе истёкший вопрос снова выглядел бы свежим. Нечитаемый файл вопросом не
        // считается: он неотличим от небывшего, и новый вопрос о нём честнее.
        let pending = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<ApprovalRequest>(&raw).ok());
        match &pending {
            // На вопрос не ответили, но никто не отказывал: это истечение окна, а не
            // запрет политикой, и в расследовании вещи разные (В4, закон 6).
            Some(asked) if expired(&asked.expires_at) => return Err(SessionError::ApprovalTimeout),
            // Вопрос ещё ждёт ответа: не переписываем — `requested_at` уехал бы из-под
            // человека, который на него смотрит, а окно продлевалось бы само собой.
            Some(_) => return Ok(Some(Verdict::Suspended)),
            None => {}
        }
        // Права на этом пути нет ни в одном исходе, но вопрос обязан ЛЕЖАТЬ на диске:
        // `Suspended` без файла неотличим от «вопрос задан и потерян», и прогон встал бы
        // навсегда без следа. Не записался — отказ с ИМЕНЕМ причины, а не тишина.
        let asked = self.approval_request(req);
        let wire = match serde_json::to_string(&asked) {
            Ok(wire) => wire,
            Err(err) => return Ok(Some(question_not_recorded(err))),
        };
        if let Err(err) = fs::create_dir_all(&dir).and_then(|()| fs::write(&path, wire)) {
            return Ok(Some(question_not_recorded(err)));
        }
        Ok(Some(Verdict::Suspended))
    }

    /// Вопрос оператору: кто спрашивает, о каком инструменте и до какого ждут ответа.
    /// `args_hash` — хеш ПУСТОГО тела: аргументов у `CallRequest` нет, а пустая строка
    /// означала бы «не посчитали». Класс риска едет СВОИМ состоянием: `Unclassified`
    /// обязан пережить круг через inbox, а не выродиться в `R0` (`risk_wire`).
    fn approval_request(&self, req: &CallRequest) -> ApprovalRequest {
        let now = unix_secs();
        ApprovalRequest {
            run_id: self.id.clone(),
            tool: req.tool.clone(),
            risk: req.risk,
            args_hash: sha256_hex(&[]),
            requested_at: rfc3339_utc(now),
            expires_at: rfc3339_utc(now.saturating_add(answer_window_secs(req.risk))),
        }
    }

    /// Чек вызова. `None` — ТОЛЬКО `Unclassified`: в `Receipt.risk` лежит `RiskClass`,
    /// «риск неизвестен» туда не вписывается, а `R0` означал бы нулевой риск (закон 6).
    /// Отсутствие — состояние, и имя ему даёт причина в `Deny`, а не молчаливый `None`.
    fn receipt(
        &self,
        req: &CallRequest,
        verdict: &Verdict,
        output_hash: &str,
        duration_ms: u64,
    ) -> Option<Receipt> {
        let ToolRisk::Classified(risk) = req.risk else {
            return None;
        };
        Some(Receipt {
            call_id: call_id(&self.id, &req.tool),
            tool: req.tool.clone(),
            module: req.module.clone(),
            verdict: verdict.clone(),
            risk,
            input_hash: sha256_hex(&[]),
            output_hash: output_hash.to_owned(),
            // Всегда ложь: этот слой не усекает и не решает, влезает ли вывод в порог,
            // потому что калибровки у него нет. Правду про усечение ставит 12c.
            truncated: false,
            duration_ms,
            ts: rfc3339_utc(unix_secs()),
        })
    }
}

/// Вопрос, который не удалось записать: отказ с названной причиной. `Suspended` без
/// вопроса на диске — это «спросили и потеряли», и прогон об этом не узнает.
fn question_not_recorded(cause: impl std::fmt::Display) -> Verdict {
    Verdict::Deny {
        reason: format!("question_not_recorded: {cause}"),
    }
}

/// Окно ответа в секундах: сутки, для R4 — четыре часа (окно объявлено в `gate::decide`).
/// R4 — внешний наблюдаемый эффект: решение, принятое на следующие сутки, относится уже
/// к другому поступку. `Unclassified` — сутки, как у любого класса, кроме R4.
fn answer_window_secs(risk: ToolRisk) -> i64 {
    match risk {
        ToolRisk::Classified(RiskClass::R4) => 4 * 3_600,
        ToolRisk::Classified(_) | ToolRisk::Unclassified => 24 * 3_600,
    }
}

/// Истекло ли окно ответа. `None` от разбора — метка не наша, и истечения мы не
/// наблюдали: назвать его таймаутом значило бы приписать причину, которой не было.
/// Права такой путь всё равно не даёт — вопрос перезаписывается, инструмент не зовётся.
fn expired(expires_at: &str) -> bool {
    match unix_secs_from_rfc3339(expires_at) {
        Some(deadline) => deadline <= unix_secs(),
        None => false,
    }
}

/// Отказ по монтажу, до `decide`. Причина различает «отсеян монтажом» и «инструмента
/// нет»: второе не должно читаться как «политика запретила».
fn mount_refusal(plan: &MountPlan, tool: &str) -> Option<String> {
    if plan.mounted.iter().any(|name| name == tool) {
        return None;
    }
    let why = match plan.skipped.iter().find(|(name, _)| name == tool) {
        Some((_, reason)) => format!(" — отсеян монтажом: {reason:?}"),
        None => String::new(),
    };
    Some(format!(
        "not_mounted: инструмент {tool} не смонтирован{why}"
    ))
}

/// SHA-256 в hex: хешируется и пустое тело, потому что «вывода не было» и «хеш не
/// посчитали» — разные вещи, и пустой строкой они слились бы.
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Идентификатор вызова: `Event.id` в потоке уникален, а `append` отдаёт `Conflict` на
/// повторе, поэтому два одинаковых отказа в одном прогоне не должны делить `id`. Счётчика
/// отказов у сессии нет — `calls` растёт только на состоявшихся вызовах.
fn call_id(run: &str, tool: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{run}-{nanos}-{tool}")
}

/// Секунды Unix для `Receipt.ts`. `unwrap_or_default`, а не паника: часы раньше эпохи —
/// дефект машины, и в чеке это видно как 1970 год, а не как «время неизвестно».
fn unix_secs() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    i64::try_from(secs).unwrap_or(i64::MAX)
}

/// Время чека в RFC 3339 UTC: `Event` сравнивает времена строками, а пустое или
/// выдуманное время неотличимо от времени вызова. Форматтер свой, потому что
/// `Cargo.toml` вне `files_allowed`, а крейт с датой к гейту не подключён.
fn rfc3339_utc(secs: i64) -> String {
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (rest / 3_600, (rest % 3_600) / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Год, месяц и день по дню от эпохи: переносы и века считаются, а не берутся из таблицы,
/// которую обязан проверять глаз.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Секунды Unix из метки времени в формате `rfc3339_utc` (`YYYY-MM-DDTHH:MM:SSZ`).
/// `None` — строка не в том формате, который печатает эта же сессия: угадывать смысл
/// чужой метки значило бы решить судьбу прогона по записи, которую никто не писал.
fn unix_secs_from_rfc3339(stamp: &str) -> Option<i64> {
    let body = stamp.strip_suffix('Z')?;
    let bytes = body.as_bytes();
    // Ровно формат, который печатает `rfc3339_utc`: 19 символов и разделители на своих
    // местах. `2026-9-29T…` и `…+02:00` — другие времена, и принять их за наши значило бы
    // сравнить срок с догадкой.
    if bytes.len() != 19 || bytes[10] != b'T' {
        return None;
    }
    for (at, want) in [(4, b'-'), (7, b'-'), (13, b':'), (16, b':')] {
        if bytes[at] != want {
            return None;
        }
    }
    let field = |range: std::ops::Range<usize>| body.get(range)?.parse::<i64>().ok();
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        field(0..4),
        field(5..7),
        field(8..10),
        field(11..13),
        field(14..16),
        field(17..19),
    ) else {
        return None;
    };
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Дни от эпохи по календарной дате — обратная к `civil_from_days`, а не таблица: пара
/// «дни ↔ дата» обязана быть взаимно обратной, иначе окно ответа съезжает, а вопрос с
/// истёкшим сроком выглядит свежим.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Итог вызова, дошедший до вызывающей стороны.
#[derive(Debug)]
pub struct CallOutcome {
    /// Решение. `Deny` и `Suspended` — успешный проход, а не сбой сессии.
    pub verdict: Verdict,
    /// Усечение вывода, если вызов состоялся и вывод не поместился в порог.
    pub shaped: Option<Shaped>,
    /// Чек вызова. `Deny` — тоже чек: «тихая» попытка не видна в расследовании, а вызов
    /// без следа неотличим от небывшего. `None` — риск вызова неизвестен, и класс в чек
    /// не вписывается; имя состоянию даёт причина отказа.
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
