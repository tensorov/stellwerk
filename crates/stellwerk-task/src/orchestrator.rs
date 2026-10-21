//! # Оркестратор запуска: единственный production-путь к движку
//!
//! Здесь сходятся три вещи, которые до N1-03 жили порознь: песочница собирает
//! `bwrap`-команду, окружение очищается и наполняется только разрешёнными
//! переменными, а исполняет всё это адаптер. Порядок обязателен: `Sandbox::command`
//! вызывается ДО `EngineAdapter::session`, и результат передаётся в него. Адаптер
//! не собирает команду сам — иначе `bwrap` и `env_clear` остаются декорацией, а не
//! гарантией (ADR-0016).
//!
//! Второго production-пути запуска нет: `SandboxWorker` — единственная реализация
//! `Worker` в этом крейте, и она не собирает команду сама ни в одной ветке.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use stellwerk_core::contract::{NetworkPolicy, RiskClass};
use stellwerk_gateway::permission::{Permission, PermissionError, ToolRisk, RISK_PERMISSION};
use thiserror::Error;

use crate::m5::sandbox_bwrap::Sandbox;
use crate::m5::secrets::{AgentEnv, SecretError, Token};
use crate::m5::EngineAdapter;
use crate::runner::{
    stop_reason_from, verdict_for, Call, ExecOutcome, Progress, RiskRegistry, RunOutcome,
    StopFacts, Worker, WorkerError,
};

// ═══════════════════════════════════════════════════════════════════════════
// Реестр риска: класс инструмента живёт в КОДЕ, а не в конфиге репозитория
// ═══════════════════════════════════════════════════════════════════════════

/// Запись реестра: какому модулю принадлежит инструмент и каков его класс риска.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolEntry {
    /// Модуль — владелец инструмента.
    pub module: &'static str,
    /// Имя инструмента.
    pub tool: &'static str,
    /// Класс риска, назначенный операции.
    pub class: RiskClass,
}

/// Статический реестр рисков.
#[derive(Debug, Clone, Copy)]
pub struct StaticRegistry(pub &'static [ToolEntry]);

impl RiskRegistry for StaticRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        risk_for_in(self.0, tool, module)
    }
}

/// Реестр инструментов Stellwerk — единственный авторитет о классе риска.
///
/// Живёт в КОДЕ, а не в конфиге репозитория: конфиг правится тем же PR, что
/// добавляет инструмент, и автор выдал бы разрешение сам себе (ADR-0017).
/// Классы назначены по смыслу операции (класс принадлежит операции, не модулю),
/// а не скопированы из конфига. Инструмент вне этого списка — не «R0 по
/// умолчанию», а `Unclassified`: неизвестное не превращается в разрешённое
/// чтение (закон 6).
///
/// Здесь только инструменты, существующие в production. Фикстурный агент CLI
/// (`stellwerk-cli run`) сюда НЕ входит: свой класс он объявляет в реестре,
/// который CLI собирает у себя (`main.rs`), иначе фикстура получала бы из
/// production-реестра разрешение, выданное ею же самою.
pub const TOOL_REGISTRY: &[ToolEntry] = &[
    // gate_report — снимок для чтения, побочного эффекта нет.
    ToolEntry {
        module: "polyforge",
        tool: "gate_report",
        class: RiskClass::R0,
    },
    // gate_evaluate — считает вердикт и меняет состояние гейта в границах прогона.
    ToolEntry {
        module: "polyforge",
        tool: "gate_evaluate",
        class: RiskClass::R1,
    },
    // evidence_append — добавляет запись в журнал доказательств: локальная запись.
    ToolEntry {
        module: "polyforge",
        tool: "evidence_append",
        class: RiskClass::R1,
    },
    // evidence_verify — связывает ToolAttestation, то есть трогает корень доверия.
    ToolEntry {
        module: "polyforge",
        tool: "evidence_verify",
        class: RiskClass::R3,
    },
];

pub const PRODUCTION_REGISTRY: StaticRegistry = StaticRegistry(TOOL_REGISTRY);

fn risk_for_in(
    registry: &[ToolEntry],
    tool: &str,
    module: &str,
) -> Result<ToolRisk, PermissionError> {
    let Some(entry) = registry
        .iter()
        .find(|entry| entry.tool == tool && entry.module == module)
    else {
        return Ok(ToolRisk::Unclassified);
    };
    class_covered_in(entry.class, &RISK_PERMISSION).map(|()| ToolRisk::Classified(entry.class))
}

/// Класс риска инструмента — из реестра Stellwerk.
///
/// Инструмент вне реестра даёт `Unclassified`, а не `R0`: иначе незнакомый
/// инструмент неотличим от разрешённого чтения. Класс известного инструмента
/// сверяется с таблицей вердиктов; отсутствие класса там — это `TableIncomplete`
/// (дефект крейта), который здесь не глушится тихим отказом.
///
/// Конфигурация репозитория не читается вовсе — это видно по сигнатуре.
pub fn risk_for(tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
    risk_for_in(TOOL_REGISTRY, tool, module)
}

/// Покрыт ли класс таблицей вердиктов `table`.
///
/// Таблица передаётся явно, чтобы отсутствие класса было наблюдаемым: рабочий
/// путь подставляет сюда ту же `RISK_PERMISSION`, что и gateway (копии нет), а
/// тест — заведомо неполную таблицу. Производственная `RISK_PERMISSION` сегодня
/// полна (R0..R5), поэтому без этой двери `TableIncomplete` был бы недостижим.
pub fn class_covered_in(
    class: RiskClass,
    table: &[(RiskClass, Permission)],
) -> Result<(), PermissionError> {
    if table.iter().any(|(candidate, _)| *candidate == class) {
        Ok(())
    } else {
        Err(PermissionError::TableIncomplete { risk: class })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Входной сетевой гейт
// ═══════════════════════════════════════════════════════════════════════════

/// Отказ входного сетевого гейта. Причина обязательна: «отказано» без неё
/// неотличимо от «не проверяли» (закон 6).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("network input gate refused `{url}`: {reason}")]
pub struct NetworkRefusal {
    /// Адрес, который прогон пытался заявить.
    pub url: String,
    /// Почему политика его не пропустила.
    pub reason: String,
}

/// Разрешено ли запуску ПОПЫТАТЬСЯ выйти по адресу — ДО старта процесса.
///
/// Это гейт по ВХОДУ, а не вторая блокировка egress: `Sandbox::argv` уже несёт
/// `--unshare-net`, поэтому у песочницы сети нет вовсе, и блокировать дважды
/// нечего. Здесь проверяется лишь то, что прогону РАЗРЕШЕНО попытаться. Разбор
/// хоста не дублируется — он живёт в `NetworkPolicy::allows`; тот же вызов
/// отвергает пустую строку при `Deny` (инвариант контракта).
pub fn network_gate(policy: &NetworkPolicy, url: &str) -> Result<(), NetworkRefusal> {
    if policy.allows(url) {
        Ok(())
    } else {
        Err(NetworkRefusal {
            url: url.to_owned(),
            reason: format!("policy {policy:?} does not permit this url"),
        })
    }
}

/// Сетевой замысел запуска. «Не заявлен» — отдельное состояние, а не «разрешено
/// всё»: если выхода не заявлено, гейтировать нечего (закон 6).
#[derive(Debug, Clone)]
enum NetworkIntent {
    /// Прогон не заявлял сетевой выход.
    NotRequested,
    /// Прогон собирается попытаться выйти по адресу под данной политикой.
    Attempt { policy: NetworkPolicy, url: String },
}

/// Вердикт сетевого гейта внутри записи. Три разных исхода, а не «ок/не ок».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkVerdict {
    /// Сетевой выход не заявлен.
    NotRequested,
    /// Адрес разрешён политикой.
    Allowed(String),
    /// Адрес отвергнут политикой.
    Refused(String),
}

/// Запись входного гейта, снятая один раз, до старта процесса.
///
/// Класс риска ЗАХВАЧЕН здесь: чтение не пересчитывает его по реестру, иначе
/// запись показывала бы сегодняшний реестр, а не тот, которым гейтировали (тот
/// же захват, что у `ResolvedExecution`, ADR-0015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRecord {
    /// Класс риска из реестра Stellwerk.
    pub risk: ToolRisk,
    /// Вердикт сетевого гейта.
    pub network: NetworkVerdict,
}

/// Состояние гейта. «Ещё не гейтировали», «решено» и «отказано» — три РАЗНЫХ
/// состояния, а не «есть/нет»: отказанный запуск не имеет записи, и это не то
/// же самое, что запуск без гейта (закон 6).
#[derive(Debug, Clone)]
enum GateState {
    /// `exec` ещё не начинался.
    Pending,
    /// Гейт снят и запись захвачена.
    Decided(GateRecord),
    /// Гейт отказал; причина сохранена.
    Refused(String),
}

/// Источник переменных окружения агента. Два РАЗНЫХ состояния, а не «есть список /
/// нет списка»: явный набор — это решение вызывающего, а белый список собирается
/// существующим `AgentEnv::assemble`. Подмена одного другим молча отбросила бы
/// проверки повторного имени и утечки токена (закон 6).
enum Environment {
    /// Явный набор: что перечислил вызывающий, то и попадёт в процесс.
    Explicit(Vec<(String, String)>),
    /// Сборка по белому списку через `AgentEnv::assemble`, а не наследование окружения.
    Allowlisted {
        allowlist: Vec<String>,
        host: Vec<(String, String)>,
        token: Token,
    },
}

/// Аргументы запуска Pi: ровно одно расширение и ЯВНЫЙ список инструментов.
///
/// `--offline` убирает сеть, `--no-session` — запись сессии, `--no-extensions` —
/// обнаружение чужих расширений, `--no-context-files` — подхват `AGENTS.md`.
/// `--api-key` не передаётся никогда: argv виден в `/proc` любому в namespace,
/// поэтому ключ остаётся на хосте и в процесс не попадает.
pub fn pi_transport_args(extension: &Path, tools: &[&str]) -> Vec<String> {
    [
        "--offline",
        "--no-session",
        "--no-extensions",
        "--no-context-files",
        "-e",
        &extension.display().to_string(),
        "--tools",
        &tools.join(","),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Воркер, запускающий движок ВНУТРИ песочницы через адаптер.
///
/// Поля закрыты: программу, аргументы, бюджет и разрешённые переменные задаёт
/// вызывающий при сборке, а не окружение процесса. Один и тот же воркер обязан
/// давать один и тот же запуск в любом терминале.
pub struct SandboxWorker<A> {
    adapter: A,
    sandbox: Sandbox,
    program: PathBuf,
    args: Vec<String>,
    budget: Duration,
    environment: Environment,
    spawned: AtomicUsize,
    /// Замер последнего запуска. `Worker::exec` возвращает `ExecOutcome` с одним лишь
    /// текстом (сигнатура из M4 не переписывается), а `evaluate()` требует `RunOutcome`;
    /// поэтому измерение живёт здесь и выдаётся через `last_outcome`/`last_refusal`.
    measure: Arc<Mutex<Measure>>,
    /// Сетевой замысел запуска, если он заявлен. Без заявления гейтировать нечего.
    network: NetworkIntent,
    /// Файл хостового unix-сокета канала модели. `None` — канал не запрошен; это
    /// отличается от «канал недоступен», и argv тогда собирается как без канала.
    socket: Option<PathBuf>,
    /// Запись входного гейта. Захвачена в начале `exec`, до старта процесса.
    gate: Arc<Mutex<GateState>>,
    /// Доверенный реестр рисков. Один и тот же реестр питает `preflight` и
    /// `runner::authorize`: если бы решение брало риск из другого места, запись
    /// гейта осталась бы декорацией, не влияющей на Allow/Deny.
    registry: Box<dyn RiskRegistry>,
}

/// Состояние замера последнего запуска. Три РАЗНЫХ состояния, а не «есть/нет»: «замера
/// ещё не было», «поток измерен» и «поток отвергнут» чинятся по-разному. Слить
/// отвергнутый ввод с отсутствием прогона значило бы выдать неразобранное за
/// неслучившееся (закон 6).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Measure {
    /// Воркер ещё не довёл запуск до замера.
    Pending,
    /// Поток разобран: счётчики и вердикт посчитаны.
    Measured(RunOutcome),
    /// Поток отвергнут: строка не является Pi-событием. Причина сохранена.
    Refused(String),
}

impl<A> SandboxWorker<A> {
    /// Собрать воркер. `env` — ЯВНЫЙ набор переменных: после `env_clear` в окружении
    /// агента не остаётся ничего, кроме перечисленного здесь. Пустой список — это
    /// «переменных нет», а не «переменные не заданы».
    pub fn new(
        adapter: A,
        sandbox: Sandbox,
        program: impl Into<PathBuf>,
        args: Vec<String>,
        budget: Duration,
        env: Vec<(String, String)>,
    ) -> Self {
        Self {
            adapter,
            sandbox,
            program: program.into(),
            args,
            budget,
            environment: Environment::Explicit(env),
            spawned: AtomicUsize::new(0),
            measure: Arc::new(Mutex::new(Measure::Pending)),
            network: NetworkIntent::NotRequested,
            socket: None,
            gate: Arc::new(Mutex::new(GateState::Pending)),
            registry: Box::new(PRODUCTION_REGISTRY),
        }
    }

    /// Заменить доверенный реестр рисков. Производственный путь использует
    /// `PRODUCTION_REGISTRY`; тесты подставляют свой, чтобы класс инструмента
    /// задавался реестром, а не самодекларацией вызова.
    pub fn with_registry(mut self, registry: impl RiskRegistry + 'static) -> Self {
        self.registry = Box::new(registry);
        self
    }

    /// Заявить сетевой выход, который запуск СОБИРАЕТСЯ попробовать. Проверка —
    /// в `exec`, до старта процесса. Без этого вызова гейтировать нечего, и это
    /// отличается от «политика разрешила»: незаявленный выход даёт `NotRequested`.
    pub fn with_network(mut self, policy: NetworkPolicy, url: impl Into<String>) -> Self {
        self.network = NetworkIntent::Attempt {
            policy,
            url: url.into(),
        };
        self
    }

    /// Отдать внутрь песочницы файл хостового unix-сокета (канал модели). Здесь только
    /// заявка: путь проверяет и монтирует `Sandbox::command_with_socket` — до запуска.
    pub fn with_socket(mut self, socket: impl Into<PathBuf>) -> Self {
        self.socket = Some(socket.into());
        self
    }

    /// Заявить ОБА канала сразу: каталог сокетов монтируется на чтение целиком, поэтому
    /// `gate.sock` виден рядом с `model.sock` без правки песочницы. Разные каталоги —
    /// отказ ДО запуска: иначе второй канал не виден, и «гейта нет» стало бы тишиной.
    pub fn with_sockets(
        mut self,
        model: impl Into<PathBuf>,
        gate: impl Into<PathBuf>,
    ) -> Result<Self, WorkerError> {
        let (model, gate) = (model.into(), gate.into());
        let dir = model
            .parent()
            .filter(|dir| Some(*dir) == gate.parent())
            .map(Path::to_path_buf)
            .ok_or_else(|| WorkerError("каналы модели и гейта в разных каталогах".to_owned()))?;
        if !dir.is_dir() {
            return Err(WorkerError(format!(
                "каталог каналов отсутствует: {}",
                dir.display()
            )));
        }
        if dir.starts_with(&self.sandbox.work) || dir.starts_with(&self.sandbox.tmp) {
            return Err(WorkerError("каталог каналов внутри work/tmp".to_owned()));
        }
        self.socket = Some(model);
        if !self.sandbox.read_only.contains(&dir) {
            self.sandbox.read_only.push(dir);
        }
        Ok(self)
    }

    /// Заменить окружение сборкой по белому списку: имена, попадающие в процесс, берутся
    /// из `allowlist`, а не из окружения хоста. Сборка (`AgentEnv::assemble`) выполняется
    /// в `exec` до старта процесса, поэтому отказ по повторному имени или по токену
    /// отменяет запуск, а не декорирует его.
    pub fn with_allowlist(
        mut self,
        allowlist: &[&str],
        host: &[(String, String)],
        token: &Token,
    ) -> Self {
        self.environment = Environment::Allowlisted {
            allowlist: allowlist.iter().map(|name| (*name).to_owned()).collect(),
            host: host.to_vec(),
            token: token.clone(),
        };
        self
    }

    /// Захваченная запись входного гейта, если он снят и не отказал. `None` —
    /// гейта ещё не было ЛИБО он отказал; это не одно и то же, и отказанная
    /// причина видна через `last_gate_refusal`.
    pub fn last_gate(&self) -> Option<GateRecord> {
        match &*self.lock_gate() {
            GateState::Decided(record) => Some(record.clone()),
            GateState::Pending | GateState::Refused(_) => None,
        }
    }

    /// Почему входной гейт отказал. `None` — отказа не было.
    pub fn last_gate_refusal(&self) -> Option<String> {
        match &*self.lock_gate() {
            GateState::Refused(reason) => Some(reason.clone()),
            GateState::Pending | GateState::Decided(_) => None,
        }
    }

    /// Достать замок гейта, переживший панику потока с ним: отравление — это
    /// «другой поток упал», а не «гейта не было» (закон 6).
    fn lock_gate(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Снять входной гейт: класс из реестра и вердикт сети. Отказ означает, что
    /// процесс не стартует. `Unclassified` отказом НЕ является: решение о нём
    /// принимает слой разрешений (`permission_for`), а не воркер, иначе два
    /// разных отказа слились бы в один.
    fn preflight(&self, tool: &str, module: &str) -> Result<GateRecord, String> {
        let risk = self
            .registry
            .risk_of(tool, module)
            .map_err(|err| format!("risk registry is incomplete: {err}"))?;
        let network = match &self.network {
            NetworkIntent::NotRequested => NetworkVerdict::NotRequested,
            NetworkIntent::Attempt { policy, url } => match network_gate(policy, url) {
                Ok(()) => NetworkVerdict::Allowed(url.clone()),
                Err(refusal) => return Err(refusal.to_string()),
            },
        };
        Ok(GateRecord { risk, network })
    }

    /// Переменные, которые попадут в процесс агента. Явный набор отдаётся как есть; белый
    /// список собирается существующим `AgentEnv::assemble`, и отсутствующее имя НЕ
    /// подставляется пустым значением: `NAME=` читалось бы как «задано, но пусто»
    /// (закон 6). Отказ сборки — это отказ запуска, а не украшение.
    fn agent_environment(&self) -> Result<Vec<(String, String)>, SecretError> {
        match &self.environment {
            Environment::Explicit(pairs) => Ok(pairs.clone()),
            Environment::Allowlisted {
                allowlist,
                host,
                token,
            } => {
                let names: Vec<&str> = allowlist.iter().map(String::as_str).collect();
                let assembled = AgentEnv::assemble(&names, host, token)?;
                Ok(allowlist
                    .iter()
                    .filter_map(|name| {
                        assembled
                            .value(name)
                            .map(|value| (name.clone(), value.to_owned()))
                    })
                    .collect())
            }
        }
    }

    /// Последний измеренный прогон. `None` — замер не состоялся (ещё не запускали или
    /// поток отвергнут); отвергнутая причина видна через `last_refusal`, и это не то же
    /// самое, что пустой поток: у пустого есть `RunOutcome` с `Verdict::InvalidLoop`.
    pub fn last_outcome(&self) -> Option<RunOutcome> {
        match &*self.lock_measure() {
            Measure::Measured(outcome) => Some(*outcome),
            Measure::Pending | Measure::Refused(_) => None,
        }
    }

    /// Почему поток последнего запуска отвергнут. `None` — отказа не было.
    pub fn last_refusal(&self) -> Option<String> {
        match &*self.lock_measure() {
            Measure::Refused(reason) => Some(reason.clone()),
            Measure::Pending | Measure::Measured(_) => None,
        }
    }

    /// Достать замок, переживший панику потока с ним. Отравление — это «другой поток
    /// упал, держа замок», а не «замера не было»: терять уже записанный замер из-за
    /// этого значило бы выдать отсутствие данных за отсутствие прогона (закон 6).
    fn lock_measure(&self) -> std::sync::MutexGuard<'_, Measure> {
        self.measure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Разобрать stdout прогона в измеренный `RunOutcome`.
///
/// Строки идут через `Progress::absorb`, счётчики превращаются в вердикт через
/// `verdict_for`; пороги берутся из раннера (`GATE_REQUIRE_*`), своих чисел здесь нет.
/// Битая строка — отказ с номером и текстом, а не молча пропущенный шаг: потерянный
/// шаг неотличим от прогона без шага (закон 6).
fn measure(stdout: &str) -> Result<RunOutcome, WorkerError> {
    let mut progress = Progress::default();
    for line in stdout.lines() {
        Progress::absorb(&mut progress, line)
            .map_err(|e| WorkerError(format!("worker stream is not a Pi event: {e}")))?;
    }
    // Вызывается только на коде 0: таймаут и ненулевой выход отсекает `session` и ветка
    // ниже, поэтому `timed_out`/`exit_code` здесь — факты чистого завершения.
    let stop_reason = stop_reason_from(StopFacts {
        timed_out: false,
        exit_code: 0,
        named: progress.stop_reason,
    });
    Ok(RunOutcome {
        stop_reason,
        tool_steps: progress.tool_steps,
        turns: progress.turns,
        total_input_tokens: progress.total_input_tokens,
        verdict: verdict_for(stop_reason, progress.tool_steps, progress.turns),
    })
}

impl<A: EngineAdapter + Sync> Worker for SandboxWorker<A> {
    fn exec(
        &self,
        _workspace_ref: &str,
        call: &Call,
    ) -> Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + '_>> {
        // Идентичность вызова копируется ДО асинхронного блока: иначе его заимствование
        // связало бы будущее с временем жизни `call`, а контракт `Worker::exec` привязывает
        // его к `&self`.
        let tool = call.tool.clone();
        let module = call.module.clone();
        Box::pin(async move {
            // Входной гейт — ДО `spawned`: отказанный запуск не стартует процесс и
            // не считается попыткой запуска.
            match self.preflight(&tool, &module) {
                Ok(record) => *self.lock_gate() = GateState::Decided(record),
                Err(reason) => {
                    *self.lock_gate() = GateState::Refused(reason.clone());
                    return Err(WorkerError(reason));
                }
            }
            // Окружение агента — ДО `spawned` и до старта процесса: отказ сборки
            // (повтор имени, токен в белом списке) обязан отменить запуск, а не
            // состояться после него. `authorize` стоит ещё раньше, в `runner::run_once`.
            let environment = self
                .agent_environment()
                .map_err(|err| WorkerError(format!("agent environment refused: {err}")))?;
            self.spawned.fetch_add(1, Ordering::SeqCst);
            // Подготовленный запуск: `bwrap`-argv собирает песочница, а не адаптер.
            let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
            let mut launch = self
                .sandbox
                .command_with_socket(
                    &self.sandbox.work,
                    &self.program,
                    &args,
                    self.budget,
                    self.socket.as_deref(),
                )
                .map_err(|e| WorkerError(format!("sandbox refused the run: {e}")))?;
            // Окружение задаётся ДО spawn: пустое, затем только разрешённые переменные.
            launch.env_clear();
            for (key, value) in &environment {
                launch.env(key, value);
            }
            let outcome = self
                .adapter
                .session(launch, self.budget)
                .map_err(|e| WorkerError(format!("engine session failed: {e}")))?;
            // Ненулевой код и убийство сигналом — РАЗНЫЕ отказы, и ни один не успех:
            // иначе упавший прогон отчитался бы `Succeeded` (закон 6).
            match outcome.code {
                Some(0) => {
                    // Код 0 — факт процесса, а не годность прогона: измеряем поток,
                    // чтобы вызывающий увидел `RunOutcome`, а не голый текст.
                    match measure(&outcome.stdout) {
                        Ok(measured) => *self.lock_measure() = Measure::Measured(measured),
                        Err(WorkerError(reason)) => *self.lock_measure() = Measure::Refused(reason),
                    }
                    Ok(ExecOutcome {
                        output: outcome.stdout,
                    })
                }
                Some(code) => Err(WorkerError(format!(
                    "process exited with code {code}: {}",
                    outcome.stderr.trim()
                ))),
                None => Err(WorkerError(format!(
                    "process was killed by a signal: {}",
                    outcome.stderr.trim()
                ))),
            }
        })
    }

    fn spawned(&self) -> usize {
        self.spawned.load(Ordering::SeqCst)
    }

    fn registry(&self) -> &dyn RiskRegistry {
        self.registry.as_ref()
    }
}
