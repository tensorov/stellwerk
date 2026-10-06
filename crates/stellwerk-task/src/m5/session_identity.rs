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

use std::path::{Path, PathBuf};
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

    /// Набор инструментов сессии — исчерпывающий.
    pub fn tools(&self) -> &ToolSet {
        &self.tools
    }
}
