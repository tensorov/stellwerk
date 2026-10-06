//! # Идентичность сессии: рабочий каталог, явный набор инструментов, лимит, идентификатор
//!
//! Субагент не должен уметь больше родителя. Проверка на входе дешёвая, а её отсутствие
//! дорого: расширенный субагент читает то, что родителю читать не положено, и это не
//! всплывает до следующего инцидента.
//!
//! Набор инструментов здесь — СПИСОК, а не запрет. «Разрешено, если не запрещено» в описании
//! сессии выразить нечем: у движка нет места, где этот список мог бы вырасти незаметно.
//! Отсюда и отказ целиком, и запрет усечения — обе вещи нужны для одного: чтобы укороченный
//! набор нельзя было принять за успешный запуск с другими правами.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

/// Codemode не встроен в ядро Pi, а включается перечислением
/// (`docs/PI_1_0_2_PROBE.md`, п. a), поэтому «включён по умолчанию» быть не может. Имя
/// приведено константой: проверка «а нет ли его в наборе» обязана искать ровно то же самое,
/// что и запрет включать.
pub const CODEMODE: &str = "codemode";

/// Отказ на границе прав. Варианты не сливаются: «субагент просил больше», «набор потерял
/// хвост», «codemode подключился сам» и «источник вызова не опознан» чинятся по-разному,
/// и молчаливое слияние отдало бы «нет данных» под видом отказа.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    /// Codemode перечислен среди инструментов сессии. Его там быть не может вовсе.
    #[error("`{tool}` is not available in a managed session: it is enabled by listing, never by default")]
    CodemodeInSet { tool: String },
    /// Codemode появился сам — через MCP, уже после загрузки конфигурации. Отсутствие его в
    /// стартовом наборе доказательством не было.
    #[error("codemode attached after the configuration load via {via}: an unverified `off` is an unverified `on`")]
    CodemodeAttached { via: &'static str },
    /// Выключение не проверено после загрузки конфигурации. Это отсутствие данных, а не
    /// «выключено»: флаг запуска доказательством не является.
    #[error("codemode was never verified after the configuration load: `off` by launch flag proves nothing")]
    CodemodeUnverified,
    /// Идентификатор пуст. «Работал кто-то без имени» при разборе не отличить от «не работал
    /// никто», а по идентификатору восстанавливается, кто с какими правами работал.
    #[error("session id is blank: without it nobody can be reconstructed as the worker")]
    Unnamed,
    /// Обещанное число инструментов не совпало с переданным. Усечение запрещено: тихая
    /// обрезка выглядит как успешный запуск с другими правами.
    #[error("tool set was truncated: {promised} promised, {given} distinct given")]
    Truncated { promised: usize, given: usize },
    /// Запрос шире родителя. Отклоняется ЦЕЛИКОМ: частичная выдача превратила бы отказ в
    /// утечку остатка набора.
    #[error("subagent asked for tools its parent does not hold: {absent:?} out of {requested:?}")]
    Escalation {
        requested: Vec<String>,
        absent: Vec<String>,
    },
    /// Инструмента нет в наборе. Не «запрещён» — именно недоступен: список исчерпывающий.
    #[error("`{tool}` is not in the tool set {held:?}: absence is the default, not permission")]
    NoSuchTool { tool: String, held: Vec<String> },
    /// Источник вызова помечен, но не опознан. Молчаливое «ну значит без родителя» выдало бы
    /// вложенный вызов за прямой.
    #[error("call source is unrecognised: `parentToolCallId` = {parent_tool_call_id:?} identifies no parent")]
    UnrecognisedSource { parent_tool_call_id: String },
    /// Путь ушёл из рабочего каталога после нормализации `..`.
    #[error(
        "`{path}` leaves the working directory `{root}`: refused before it reaches the engine"
    )]
    Outside { root: PathBuf, path: PathBuf },
}

/// Явный набор инструментов. Не «список запрещённых», а исчерпывающий перечень того, что
/// ЕСТЬ: инструмента нет в списке — он недоступен, и «по умолчанию разрешено, если не
/// запрещено» здесь не выразить нечем.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSet(Vec<String>);

impl ToolSet {
    /// Список, чьё обещание равно длине: кто-то уже решил, сколько инструментов будет.
    pub fn listed(tools: &[&str]) -> Result<Self, IdentityError> {
        Self::exactly(tools.len(), tools)
    }

    /// Список с ОБЕЩАННЫМ числом инструментов. Расхождение — отказ, а не укороченный список:
    /// обрезанный набор неотличим от успешного запуска с другими правами, и починить его
    /// задним числом нельзя. Повтор считается одним инструментом, поэтому обещание о двух
    /// местах под один инструмент тоже не выполнено, а тихая дедупликация выдала бы набор с
    /// правами, которых не просили.
    pub fn exactly(promised: usize, tools: &[&str]) -> Result<Self, IdentityError> {
        let names: Vec<String> = tools.iter().map(|t| (*t).to_owned()).collect();
        let given = names
            .iter()
            .enumerate()
            .filter(|(at, t)| !names[..*at].contains(t))
            .count();
        if given != promised {
            return Err(IdentityError::Truncated { promised, given });
        }
        if names.iter().any(|t| t == CODEMODE) {
            return Err(IdentityError::CodemodeInSet {
                tool: CODEMODE.to_owned(),
            });
        }
        Ok(Self(names))
    }

    /// Есть ли инструмент. Ответ «нет» означает «недоступен», а не «запрещён»: списка
    /// запретов здесь нет и быть не может.
    pub fn holds(&self, tool: &str) -> bool {
        self.0.iter().any(|t| t == tool)
    }

    /// Перечень целиком, в том порядке, в каком он уйдёт движку.
    pub fn names(&self) -> &[String] {
        &self.0
    }
}

/// Выдача субагента прав субагента: либо ВЕСЬ набор, либо отказ. Частичной выдачи нет и
/// быть не может — пересечение с родительским набором здесь не вычисляется вовсе, а тип
/// возврата не даёт вернуть укороченный список. Обрезанный по родителю набор и есть
/// утечка остатка, поэтому отказ назван виновником целиком.
pub fn grant(parent: &ToolSet, request: &[&str]) -> Result<ToolSet, IdentityError> {
    let asked = ToolSet::listed(request)?;
    let absent: Vec<String> = asked
        .names()
        .iter()
        .filter(|t| !parent.holds(t))
        .cloned()
        .collect();
    if absent.is_empty() {
        Ok(asked)
    } else {
        Err(IdentityError::Escalation {
            requested: asked.names().to_vec(),
            absent,
        })
    }
}

/// Источник вызова. Проба Pi 1.0.2 (п. a) измерена: вложенные вызовы проходят через те же
/// хуки `tool_call` и помечаются полем `parentToolCallId`. Значит прямой и вложенный
/// различаются на входе, а не разными проверками: происхождение не даёт прав.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallSource {
    /// Поле не проставлено: вызов сделала сама сессия.
    Direct,
    /// Поле проставлено: вызов сделал другой инструмент. Идентификатор обязателен — пустой
    /// не опознан, а молчаливый «значит прямой» стёр бы различие между вызовами.
    Nested { parent_tool_call_id: String },
}

/// Состояние codemode ПОСЛЕ загрузки конфигурации. Три состояния вместо флага — потому что
/// «выключено по флагу запуска» не доказывает «выключено»: codemode подключается сам через
/// MCP, и непроверенное выключение равно непроверенному включению.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodemodeState {
    /// Проверено после загрузки конфигурации: инструмента нет. Вариант назван по тому, ЧЕМ
    /// установлен, поэтому «выключено» нельзя написать мимо доказательства.
    AbsentAfterConfigLoad,
    /// Подключился сам. Виновник назван, иначе «подключилось» и «не проверено» совпали бы.
    Attached { via: &'static str },
    /// Не проверено. Отсутствие данных — не решение (закон 6).
    Unverified,
}

/// Описание сессии — ровно четыре вещи: рабочий каталог, набор инструментов, лимит времени,
/// идентификатор. Ничего больше движку не передаётся, поэтому секрет в окружении агента
/// некуда поместиться: поля закрыты, конструктор один, и параметров ровно четыре.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDescription {
    work: PathBuf,
    tools: ToolSet,
    limit: Duration,
    id: String,
}

impl SessionDescription {
    /// Собирает описание и отказывает ДО чтения ФС: отказ по составу сессии не должен
    /// зависеть от того, что каталог лежит на диске. Порядок отказов — по тяжести: сначала
    /// то, что делает сессию бессмысленной (codemode), потом то, что делает её нечитаемой
    /// (пустой идентификатор).
    pub fn new(
        id: &str,
        work: &Path,
        tools: ToolSet,
        limit: Duration,
        codemode: CodemodeState,
    ) -> Result<Self, IdentityError> {
        match codemode {
            CodemodeState::AbsentAfterConfigLoad => {}
            CodemodeState::Attached { via } => return Err(IdentityError::CodemodeAttached { via }),
            CodemodeState::Unverified => return Err(IdentityError::CodemodeUnverified),
        }
        let id = id.trim();
        if id.is_empty() {
            return Err(IdentityError::Unnamed);
        }
        Ok(Self {
            work: work.to_owned(),
            tools,
            limit,
            id: id.to_owned(),
        })
    }

    /// Рабочий каталог: ровно он уходит движку, и он приходит из описания, а не из
    /// окружения процесса.
    pub fn cwd(&self) -> &Path {
        &self.work
    }

    /// Набор инструментов сессии — исчерпывающий.
    pub fn tools(&self) -> &ToolSet {
        &self.tools
    }

    /// Лимит времени из описания. Бюджет исполнения считает вызывающий (`m5::EngineAdapter`),
    /// и подмена одного другим сделала бы два разных инцидента одним.
    pub fn limit(&self) -> Duration {
        self.limit
    }

    /// Идентификатор: по нему восстанавливается, кто с какими правами работал.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Строка журнала. Идентификатор обязателен здесь по той же причине, что и в отчёте:
    /// запись без него при разборе не отличить от «сессии не было».
    pub fn journal_line(&self, event: &str) -> String {
        let tools = self.tools.names().join(",");
        format!("session={} event={} tools={}", self.id, event, tools)
    }

    /// Отчёт о сессии: идентификатор, каталог, лимит и перечень инструментов — то есть всё
    /// описание целиком. Строится здесь, а не вызывающим: вызывающий идентификатор забыть
    /// может, и забытое неотличимо от «работал никто».
    pub fn report(&self) -> String {
        format!(
            "session={} cwd={} limit={:?} tools={}",
            self.id,
            self.work.display(),
            self.limit,
            self.tools.names().join(","),
        )
    }

    /// Путь, который движку можно передать. Выход за рабочий каталог запрещён, и проверка
    /// лексическая: `canonicalize` уронил бы ещё не существующий путь, а решить «по
    /// существованию» нельзя — тогда граница врала бы ровно там, где опасно.
    pub fn inside<'p>(&self, path: &'p Path) -> Result<&'p Path, IdentityError> {
        let root = lexical(&self.work);
        let asked = lexical(path);
        if asked.starts_with(&root) {
            Ok(path)
        } else {
            Err(IdentityError::Outside { root, path: asked })
        }
    }

    /// Проверка политики по СЫРОМУ полю `parentToolCallId` из хука `tool_call`. Один вход и
    /// для прямого, и для вложенного вызова: вложенный проходит её заново, с ИСХОДНЫМИ
    /// правами, и не наследует ничего — поэтому строже здесь может быть только требование
    /// «источник опознан», а не сама проверка. Решение одно и то же, и различие видно в
    /// единственном поле `source`.
    pub fn adjudicate(
        &self,
        tool: &str,
        parent_tool_call_id: Option<&str>,
    ) -> Result<Adjudication, IdentityError> {
        let source = CallSource::of(parent_tool_call_id)?;
        if !self.tools.holds(tool) {
            return Err(IdentityError::NoSuchTool {
                tool: tool.to_owned(),
                held: self.tools.names().to_vec(),
            });
        }
        Ok(Adjudication {
            session: self.id.clone(),
            tool: tool.to_owned(),
            source,
        })
    }
}

impl CallSource {
    /// Источник опознаётся из поля, пришедшего извне, а значит может быть и мусором. Пустой
    /// идентификатор происхождения — это НЕ прямой вызов: источник не опознан, и тихое «ну
    /// значит без родителя» стёрло бы различие, ради которого поле и помечается.
    pub fn of(parent_tool_call_id: Option<&str>) -> Result<Self, IdentityError> {
        match parent_tool_call_id {
            None => Ok(Self::Direct),
            Some(id) if id.trim().is_empty() => Err(IdentityError::UnrecognisedSource {
                parent_tool_call_id: id.to_owned(),
            }),
            Some(id) => Ok(Self::Nested {
                parent_tool_call_id: id.to_owned(),
            }),
        }
    }
}

/// Решение по вызову. Происхождение записывается, но в права не превращается: оно нужно для
/// разбора («откуда пришёл вызов»), а не для разрешения («знаю, откуда — можно»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjudication {
    /// Сессия, чьими правами проверялось.
    pub session: String,
    /// Инструмент, которому разрешено.
    pub tool: String,
    /// Откуда пришёл вызов — как в `SessionDescription::adjudicate`.
    pub source: CallSource,
}

/// Нормализация лексическая: `..` и `.` разложены руками, без ФС. Вверх за `/` не
/// поднимаемся: нечего отдавать. Отдельная от `sandbox_bwrap::normalize` — та приватна, а
/// тот файл вне этого пакета; идея не дублируется, дублируется шесть строк арифметики.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
