//! Plan surface: цель → диалог с настоящей моделью → черновик → явное утверждение
//! человеком.
//!
//! Границы этого шага:
//!
//! * Управляющая модель **не получает** raw bash/edit/write. Её конфигурация
//!   (`PLANNER_CONFIG_JSON`) выключает все инструменты; транспорт отдаёт её
//!   `opencode` через `OPENCODE_CONFIG`, не трогая проектную конфигурацию владельца.
//! * Модель **не утверждает план**. «План утверждён» в устах модели — текст;
//!   утверждение существует только как `PlanSession::approve()` по нажатию человека.
//! * Build **не исполняется**: исполнителя нет (N2-00h3b), и это названо явно
//!   (`build_hint`), а не изображено. Допуск к исполнению при этом уже
//!   проверяется по существу (`build_request`): неутверждённый план, чужой снимок и план,
//!   сделанный мимо транспорта, отвергаются здесь.
//!
//! Чистые части (`parse_reply`, `PlanSession`, `render_plan`, конфигурация)
//! проверяются без TTY и без сети (`tests/plan.rs`). Терминал и процесс модели
//! живут только в `run_plan_tui` и `LiveTransport`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use clap::ValueEnum;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::build_runner::{self, BuildOutcome, BuildRun};
use crate::host_model::ModelConfig;
use crate::planner_host::PlannerHost;
use stellwerk_task::m5::model_backend::HttpBackend;
use stellwerk_task::m5::secrets::{read_token_outside, TokenProxy};

/// Имя агента-планировщика в конфигурации транспорта.
pub const PLANNER_AGENT: &str = "stellwerk-planner";

/// Модель планировщика. Единственный живой маршрут этого окружения к DeepSeek;
/// Pi 0.87 не знает этого провайдера вовсе (`pi auth check --provider deepseek`
/// → `not_ready`), поэтому выбран OpenCode, а не Pi RPC.
pub const PLANNER_MODEL: &str = "opencode/deepseek-v4.1-flash-free";

/// Адрес и модель для режима `host`. Подтверждены владельцем; в теле запроса уходит голый
/// id — префикс `neuraldeep/` это маршрут Pi, а не поле OpenAI-совместимого API.
pub const HOST_BASE_URL: &str = "https://api.neuraldeep.ru/v1";
pub const HOST_MODEL_ID: &str = "qwen3.6-unlim-xl-noreason";

/// Дословная причина, по которой Build в этом срезе недоступен. Это не заглушка:
/// ни `model transport` (N2-00), ни блокирующая pre-execution авторизация
/// инструментов, инициированных моделью, не построены.
/// Что на самом деле происходит с Build на этом экране. Текст зависит от стадии, потому что
/// «недоступен» перестало быть правдой после N2-00h3d, а обещание «запущен» до прогона —
/// ложь того же рода.
fn build_hint(stage: Stage) -> &'static str {
    match stage {
        Stage::Approved => {
            "Build: b — запустить (агент в копии дерева; запись и оболочка запрещены политикой)"
        }
        Stage::Building => "Build: прогон идёт",
        _ => "Build: сначала утвердите план (a), затем b",
    }
}

/// Конфигурация планировщика для `opencode`: агент без мутирующих, исполняющих
/// и сетевых инструментов. Единственный включённый инструмент — scoped `read`.
///
/// `read` оставлен не для удобства, а потому что шлюз free-tier этого окружения
/// отвергает запрос, в котором не осталось ни одного read-family инструмента:
/// `{"tools": {"bash": false, ...}}` с выключенным `read` даёт
/// `403 FreeTierError: can only be used from within OpenCode`, а конфигурация с
/// одним `read` — принимается. Инвариант «нет raw bash/edit/write» сохранён:
/// `bash`, `edit`, `write`, `patch`, `task`, `webfetch` выключены, а `read`
/// ограничен рабочим каталогом (запрос вне него возвращает tool error).
pub const PLANNER_CONFIG_JSON: &str = r#"{
  "$schema": "https://opencode.ai/config.json",
  "agent": {
    "stellwerk-planner": {
      "description": "Stellwerk planner: scoped read-only inspection; no mutation, exec, or network.",
      "model": "opencode/deepseek-v4.1-flash-free",
      "tools": {
        "bash": false, "edit": false, "write": false, "read": true,
        "glob": false, "grep": false, "list": false, "patch": false,
        "webfetch": false, "task": false, "todowrite": false
      }
    }
  }
}"#;

/// Режим планировщика. `Host` — наш транспорт, `Opencode` — путь отладки промпта: ход в нём
/// идёт мимо гейта и бюджета, поэтому Build его не примет (00h3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum PlannerMode {
    #[default]
    Host,
    Opencode,
}

impl PlannerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Opencode => "opencode",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "host" => Some(Self::Host),
            "opencode" => Some(Self::Opencode),
            _ => None,
        }
    }
}

/// Какой режим действует и надо ли записать его в сессию.
///
/// Записанный режим старше флага: утверждённый план не должен менять происхождение при
/// перезапуске — иначе Build получил бы план, про который нельзя сказать, как он сделан.
/// Неизвестная запись — отказ, а не «сойдёт за host».
pub fn resolve_mode(
    recorded: Option<&str>,
    requested: PlannerMode,
) -> Result<(PlannerMode, bool), String> {
    match recorded {
        None => Ok((requested, true)),
        Some(text) => PlannerMode::parse(text)
            .map(|mode| (mode, false))
            .ok_or_else(|| format!("session records unknown planner `{text}`")),
    }
}

/// Как режим назван человеку. Отсутствие записи — отдельное состояние.
pub fn describe_planner(recorded: Option<&str>) -> String {
    match recorded {
        None => "не записан".to_string(),
        Some("host") => "host (наш транспорт)".to_string(),
        Some("opencode") => "opencode — вне транспорта".to_string(),
        Some(other) => format!("неизвестный режим `{other}`"),
    }
}

/// Черновик плана в терминах шага 2: цель, допустимые изменения, критерии приёмки,
/// проверки, ограничения исполнителя. Поля без `Default`: отсутствие поля — это
/// состояние, которое обязано быть видно, а не значение по умолчанию (закон 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDraft {
    pub goal: String,
    pub allowed_changes: Vec<String>,
    pub acceptance: Vec<String>,
    pub checks: Vec<String>,
    pub constraints: Vec<String>,
}

/// Реплика диалога.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Turn {
    User(String),
    Model(String),
}

/// Сессия планирования: сохраняется на диск целиком, чтобы после нового открытия
/// TUI были видны и диалог, и черновик, и факт утверждения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSession {
    pub repo: String,
    pub snapshot: String,
    pub goal: Option<String>,
    pub turns: Vec<Turn>,
    pub draft: Option<PlanDraft>,
    pub summary: Option<String>,
    /// Растёт на каждом изменении цели или черновика. Утверждение привязано к
    /// конкретному значению версии — поэтому правка после утверждения снова
    /// требует человека.
    pub version: u64,
    pub approved_version: Option<u64>,
    /// SHA-256 канонического текста утверждённого плана. Отдельно от `version`:
    /// счётчик ловит правку через API, дайджест — правку текста в обход счётчика.
    /// `#[serde(default)]` нужен для старых `session.json` без поля: отсутствие
    /// означает «утверждения с привязкой к тексту нет», то есть человека всё равно
    /// спросят, а не молча сочтут план утверждённым.
    #[serde(default)]
    pub approved_digest: Option<String>,
    /// Режим, которым получен план. `None` — запись отсутствует (старая сессия), и это не
    /// то же самое, что `host`: происхождение неизвестно, а не «наш транспорт».
    #[serde(default)]
    pub planner: Option<String>,
}

impl PlanSession {
    pub fn new(repo: &str, snapshot: &str) -> Self {
        Self {
            repo: repo.to_string(),
            snapshot: snapshot.to_string(),
            goal: None,
            turns: Vec::new(),
            draft: None,
            summary: None,
            version: 0,
            approved_version: None,
            approved_digest: None,
            planner: None,
        }
    }

    pub fn set_goal(&mut self, goal: &str) {
        self.goal = Some(goal.to_string());
        self.version += 1;
    }

    pub fn push_turn(&mut self, turn: Turn) {
        self.turns.push(turn);
    }

    /// Новый черновик — новая версия. Именно это делает «план изменился после
    /// утверждения» отличимым от «тот же самый план».
    pub fn set_draft(&mut self, draft: PlanDraft, summary: String) {
        self.draft = Some(draft);
        self.summary = Some(summary);
        self.version += 1;
    }

    pub fn approve(&mut self) {
        self.approved_version = Some(self.version);
        self.approved_digest = self.plan_digest();
    }

    /// Канонический текст плана — то, что человек утверждает. Порядок полей
    /// фиксирован, отсутствие summary названо явно (`<нет>`), а не пустой
    /// строкой: отсутствие — состояние (закон 6). `None` — плана нет.
    fn plan_text(&self) -> Option<String> {
        let draft = self.draft.as_ref()?;
        let mut out = String::new();
        out.push_str("summary: ");
        out.push_str(self.summary.as_deref().unwrap_or("<нет>"));
        out.push('\n');
        out.push_str("goal: ");
        out.push_str(&draft.goal);
        out.push('\n');
        for (label, items) in [
            ("allowed_changes", &draft.allowed_changes),
            ("acceptance", &draft.acceptance),
            ("checks", &draft.checks),
            ("constraints", &draft.constraints),
        ] {
            out.push_str(label);
            out.push_str(":\n");
            for item in items {
                out.push_str("- ");
                out.push_str(item);
                out.push('\n');
            }
        }
        Some(out)
    }

    /// Дайджест канонического текста плана; `None` — плана нет.
    pub fn plan_digest(&self) -> Option<String> {
        let text = self.plan_text()?;
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        Some(format!("{:x}", hasher.finalize()))
    }

    /// Привязывает утверждение к дереву: сменился снимок — утверждение сброшено.
    pub fn reconcile_snapshot(&mut self, current: &str) {
        if self.snapshot != current {
            self.snapshot = current.to_string();
            if self.draft.is_some() {
                self.version += 1;
            }
        }
    }

    pub fn approval_required(&self) -> bool {
        self.approved_version != Some(self.version) || self.approved_digest != self.plan_digest()
    }

    pub fn is_approved(&self) -> bool {
        self.draft.is_some() && !self.approval_required()
    }
}

/// Запрос на исполнение: то, что разрешено делать. Поля закрыты, конструктор один —
/// собрать запрос в обход проверок нельзя.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    pub goal: String,
    pub allowed_changes: Vec<String>,
    pub acceptance: Vec<String>,
    pub checks: Vec<String>,
}

/// Почему исполнение не разрешено. Каждая причина — отдельный вариант: «никто не утверждал»,
/// «план правили после утверждения», «дерево сдвинулось», «происхождение не записано»,
/// «план сделан вне транспорта» — это разные ответы, и сливать их в один нельзя.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRefusal {
    NoPlan,
    Unapproved { version: u64, approved: Option<u64> },
    DigestChanged { version: u64 },
    SnapshotDrift { planned: String, current: String },
    PlannerNotRecorded,
    PlannerBypassedTransport { recorded: String },
    PlannerUnknown { recorded: String },
}

impl std::fmt::Display for BuildRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlan => write!(f, "плана нет: исполняется утверждённый план, а не намерение"),
            Self::Unapproved { version, approved } => match approved {
                Some(approved) => write!(
                    f,
                    "план не утверждён: версия {version}, утверждена версия {approved}"
                ),
                None => write!(f, "план не утверждён: версия {version}, утверждения нет"),
            },
            Self::DigestChanged { version } => write!(
                f,
                "утверждение не привязано к текущему тексту плана (версия {version}): текст менялся \
                 в обход счётчика версий — утвердите заново"
            ),
            Self::SnapshotDrift { planned, current } => write!(
                f,
                "план утверждён для снимка {planned}, а репозиторий на {current}: исполнитель \
                 получил бы другое дерево"
            ),
            Self::PlannerNotRecorded => write!(
                f,
                "происхождение плана не записано: неизвестно, сделан ли он нашим транспортом"
            ),
            Self::PlannerBypassedTransport { recorded } => write!(
                f,
                "план сделан вне транспорта (`{recorded}`): такой план к исполнению не допускается"
            ),
            Self::PlannerUnknown { recorded } => write!(
                f,
                "записано незнакомое происхождение `{recorded}`: это ни host, ни opencode"
            ),
        }
    }
}

/// Можно ли исполнять этот план. Функция чистая: снимок приходит аргументом, поэтому
/// проверяется без репозитория и без диска.
pub fn build_request(
    session: &PlanSession,
    current_snapshot: &str,
) -> Result<BuildRequest, BuildRefusal> {
    let Some(draft) = session.draft.as_ref() else {
        return Err(BuildRefusal::NoPlan);
    };
    if session.approved_version != Some(session.version) {
        return Err(BuildRefusal::Unapproved {
            version: session.version,
            approved: session.approved_version,
        });
    }
    // Дайджест ловит то, что счётчик версий пропустил: текст правили через API.
    if session.approved_digest != session.plan_digest() {
        return Err(BuildRefusal::DigestChanged {
            version: session.version,
        });
    }
    if session.snapshot != current_snapshot {
        return Err(BuildRefusal::SnapshotDrift {
            planned: session.snapshot.clone(),
            current: current_snapshot.to_string(),
        });
    }
    match session.planner.as_deref() {
        Some("host") => {}
        Some("opencode") => {
            return Err(BuildRefusal::PlannerBypassedTransport {
                recorded: "opencode".to_string(),
            })
        }
        Some(other) => {
            return Err(BuildRefusal::PlannerUnknown {
                recorded: other.to_string(),
            })
        }
        None => return Err(BuildRefusal::PlannerNotRecorded),
    }
    Ok(BuildRequest {
        goal: draft.goal.clone(),
        allowed_changes: draft.allowed_changes.clone(),
        acceptance: draft.acceptance.clone(),
        checks: draft.checks.clone(),
    })
}

/// Разобранный ответ модели. `Unrecognized` — отдельное состояние, а не «план по
/// умолчанию»: угадывать смысл ответа значило бы подменить отсутствие решения
/// решением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Question { text: String },
    Plan { draft: PlanDraft, summary: String },
    Unrecognized { raw: String },
}

/// Разбор ответа модели. План или вопрос должны прийти одним JSON-объектом; он
/// может быть обёрнут в прозу, поэтому берём первый `{` и последний `}`.
pub fn parse_reply(raw: &str) -> Reply {
    let Some(value) = extract_json(raw) else {
        return Reply::Unrecognized {
            raw: raw.to_string(),
        };
    };
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("question") => match value.get("text").and_then(serde_json::Value::as_str) {
            Some(text) => Reply::Question {
                text: text.to_string(),
            },
            None => Reply::Unrecognized {
                raw: raw.to_string(),
            },
        },
        Some("plan") => match plan_from_json(&value) {
            Some((draft, summary)) => Reply::Plan { draft, summary },
            None => Reply::Unrecognized {
                raw: raw.to_string(),
            },
        },
        _ => Reply::Unrecognized {
            raw: raw.to_string(),
        },
    }
}

fn extract_json(raw: &str) -> Option<serde_json::Value> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let candidate = raw.get(start..=end)?;
    serde_json::from_str(candidate).ok()
}

fn plan_from_json(value: &serde_json::Value) -> Option<(PlanDraft, String)> {
    let summary = value.get("summary").and_then(serde_json::Value::as_str)?;
    let draft = PlanDraft {
        goal: value
            .get("goal")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        allowed_changes: path_list(value, "allowed_changes")?,
        acceptance: string_list(value, "acceptance")?,
        checks: string_list(value, "checks")?,
        constraints: string_list(value, "constraints")?,
    };
    Some((draft, summary.to_string()))
}

/// Элемент `allowed_changes` — путь. Модель иногда присылает объект `{file, change}`;
/// `file` и есть путь, а `change` — проза для человека, не для скоупа. Любая другая
/// форма (нет `file`, не строка) — отказ разбора, а не догадка.
fn path_list(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    let array = value.get(key)?.as_array()?;
    array
        .iter()
        .map(|item| match item {
            serde_json::Value::String(path) => Some(path.clone()),
            serde_json::Value::Object(object) => object
                .get("file")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            _ => None,
        })
        .collect()
}

fn string_list(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    let array = value.get(key)?.as_array()?;
    array
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

/// Неразобранный ответ модели — улика, а не мусор: экран обрезает строки, и без файла
/// причину отказа не установить.
pub fn record_unparsed(repo: &Path, raw: &str) -> std::io::Result<PathBuf> {
    let path = repo.join(".stellwerk").join("plan").join("last-reply.txt");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, raw)?;
    Ok(path)
}

/// Путь к сохранённой сессии внутри репозитория.
pub fn session_path(repo: &Path) -> PathBuf {
    repo.join(".stellwerk").join("plan").join("session.json")
}

pub fn save_session(path: &Path, session: &PlanSession) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create plan dir `{}`: {e}", dir.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(session).map_err(|e| format!("serialize plan: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("write plan `{}`: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace plan `{}`: {e}", path.display()))
}

/// Отсутствие файла — отдельное состояние (`Ok(None)`), не пустая сессия.
pub fn load_session(path: &Path) -> Result<Option<PlanSession>, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("parse plan `{}`: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read plan `{}`: {e}", path.display())),
    }
}

/// Фаза экрана. Состояния Build (`выполняется`, `проверка`) отсутствуют
/// сознательно: Build недоступен, и показывать их было бы изображением.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Goal,
    Asking,
    Answering,
    Review,
    Approved,
    Building,
    Failed,
    Cancelled,
}

/// Всё, что нужно чистому рендеру. Отдельная структура, а не длинный список
/// аргументов: рендер обязан оставаться чистым швом.
pub struct View<'a> {
    pub session: &'a PlanSession,
    pub stage: Stage,
    pub input: &'a str,
    pub model_text: &'a str,
    pub error: Option<&'a str>,
}

fn stage_label(stage: Stage) -> &'static str {
    match stage {
        Stage::Goal => "готов",
        Stage::Asking => "отвечает модель",
        Stage::Answering => "ждёт ответа человека",
        Stage::Review => "черновик",
        Stage::Approved => "утверждён",
        Stage::Building => "сборка идёт",
        Stage::Failed => "ошибка",
        Stage::Cancelled => "отменён",
    }
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        "—".to_string()
    } else {
        items.join("; ")
    }
}

/// Чистый рендер: `View` → строки. Ни терминала, ни сети.
pub fn render_plan(view: &View<'_>) -> Vec<Line<'static>> {
    let session = view.session;
    let mut lines = vec![
        Line::from(Span::styled(
            "Stellwerk — план изменения",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("repo:     {}", session.repo)),
        Line::from(format!("snapshot: {}", session.snapshot)),
        Line::from(format!("stage:    {}", stage_label(view.stage))),
        Line::from(""),
    ];

    match &session.goal {
        Some(goal) => lines.push(Line::from(format!("цель: {goal}"))),
        None => lines.push(Line::from(Span::styled(
            "цель: — (введите цель и нажмите Enter)",
            Style::default().add_modifier(Modifier::DIM),
        ))),
    }

    if !session.turns.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "диалог:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for turn in session.turns.iter().rev().take(6).rev() {
            let (who, text) = match turn {
                Turn::User(text) => ("человек", text),
                Turn::Model(text) => ("модель", text),
            };
            lines.push(Line::from(format!("  {who}: {text}")));
        }
    }
    if !view.model_text.is_empty() {
        lines.push(Line::from(format!(
            "  модель (сейчас): {}",
            view.model_text
        )));
    }

    if let Some(draft) = &session.draft {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "план:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        if let Some(summary) = &session.summary {
            lines.push(Line::from(format!("  summary: {summary}")));
        }
        lines.push(Line::from(format!("  цель: {}", draft.goal)));
        lines.push(Line::from(format!(
            "  допустимые изменения: {}",
            render_list(&draft.allowed_changes)
        )));
        lines.push(Line::from(format!(
            "  критерии приёмки:     {}",
            render_list(&draft.acceptance)
        )));
        lines.push(Line::from(format!(
            "  проверки:             {}",
            render_list(&draft.checks)
        )));
        lines.push(Line::from(format!(
            "  ограничения:          {}",
            render_list(&draft.constraints)
        )));
        let approved = session
            .approved_version
            .map_or_else(|| "—".to_string(), |v| v.to_string());
        lines.push(Line::from(format!(
            "  версия: {} / утверждена: {}",
            session.version, approved
        )));
        if session.approval_required() {
            lines.push(Line::from(Span::styled(
                "[ A ] утвердить эту версию плана (ждёт утверждения)",
                Style::default().add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "[ утверждено человеком ]",
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
    }

    lines.push(Line::from(""));
    match view.stage {
        Stage::Goal => lines.push(Line::from(format!("> {}", view.input))),
        Stage::Answering => lines.push(Line::from(format!("ответ> {}", view.input))),
        Stage::Asking => lines.push(Line::from(Span::styled(
            "модель отвечает… (Esc — отменить)",
            Style::default().add_modifier(Modifier::DIM),
        ))),
        Stage::Building => lines.push(Line::from(Span::styled(
            "агент работает в копии дерева… (прогон не отменяется)",
            Style::default().add_modifier(Modifier::DIM),
        ))),
        _ => {}
    }

    if let Some(err) = view.error {
        lines.push(Line::from(Span::styled(
            format!("ошибка: {err}"),
            Style::default().add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "ход модели: {}",
            describe_planner(session.planner.as_deref())
        ),
        Style::default().add_modifier(Modifier::DIM),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        build_hint(view.stage),
        Style::default().add_modifier(Modifier::DIM),
    )));
    lines
}

/// Событие транспорта: текст модели, отказ или завершение запроса.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    Text(String),
    Failed(String),
    Finished { session: Option<String> },
}

/// Промпт планировщику. Строгий JSON-конверт + явное «нет инструментов» +
/// запрет объявлять утверждение.
pub fn build_prompt(session: &PlanSession) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "Ты — планировщик Stellwerk. Ты не можешь читать репозиторий, менять файлы, \
         запускать команды или ходить в сеть: рабочий каталог пуст. Планируй строго \
         по тексту ниже.\n\n",
    );
    prompt.push_str(&format!("Репозиторий: {}\n", session.repo));
    prompt.push_str(&format!("Снимок (git HEAD): {}\n", session.snapshot));
    if let Some(goal) = &session.goal {
        prompt.push_str(&format!("Цель владельца: {goal}\n"));
    }
    for turn in &session.turns {
        match turn {
            Turn::User(text) => prompt.push_str(&format!("Человек: {text}\n")),
            Turn::Model(text) => prompt.push_str(&format!("Ты: {text}\n")),
        }
    }
    prompt.push_str(
        "\nОтветь СТРОГО одним JSON-объектом, без markdown.\n\
         Если данных не хватает — задай ровно один вопрос:\n\
         {\"kind\":\"question\",\"text\":\"...\"}\n\
         Если данных достаточно — верни план:\n\
         {\"kind\":\"plan\",\"summary\":\"...\",\"goal\":\"...\",\"allowed_changes\":[...],\
         \"acceptance\":[...],\"checks\":[...],\"constraints\":[...]}\n\
         allowed_changes — только пути строками (например \"src/lib.rs\"), без объектов\
         и без описаний: что именно правится, сказано в summary.\n\
         Ты НЕ утверждаешь план: утверждение делает человек.\n",
    );
    prompt
}

/// Куда уходит ход модели. Две реализации одного поведения; выбор записан в сессии, а не
/// угадан по окружению.
enum Channel {
    Opencode {
        child: Option<Child>,
        scratch: PathBuf,
    },
    Host {
        planner: Option<std::sync::Arc<PlannerHost>>,
        scratch: PathBuf,
    },
}

/// Живой транспорт к модели.
pub struct LiveTransport {
    channel: Channel,
}

impl LiveTransport {
    /// Режим `opencode`: дочерний процесс, как было до 00h2a.
    pub fn new(scratch: PathBuf) -> Self {
        Self {
            channel: Channel::Opencode {
                child: None,
                scratch,
            },
        }
    }

    pub fn with_mode(mode: PlannerMode, scratch: PathBuf) -> Self {
        match mode {
            PlannerMode::Opencode => Self::new(scratch),
            PlannerMode::Host => Self {
                channel: Channel::Host {
                    planner: None,
                    scratch,
                },
            },
        }
    }

    /// Запускает запрос. Возвращает приёмник событий; чтение идёт в отдельном
    /// потоке, поэтому TUI не зависает.
    pub fn ask(&mut self, prompt: &str) -> Result<Receiver<TransportEvent>, String> {
        match &mut self.channel {
            Channel::Opencode { child, scratch } => {
                let config = write_planner_config(scratch)?;
                let mut process = Command::new("opencode")
                    .args([
                        "run",
                        "--pure",
                        "--format",
                        "json",
                        "--agent",
                        PLANNER_AGENT,
                    ])
                    .arg(prompt)
                    .env("OPENCODE_CONFIG", &config)
                    .current_dir(scratch)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("spawn opencode: {e}"))?;
                let stdout = process
                    .stdout
                    .take()
                    .ok_or_else(|| "opencode не отдал stdout".to_string())?;
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    let mut session = None;
                    let mut failed = false;
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                            continue;
                        };
                        if let Some(id) = value.get("sessionID").and_then(serde_json::Value::as_str)
                        {
                            session = Some(id.to_string());
                        }
                        match value.get("type").and_then(serde_json::Value::as_str) {
                            Some("text") => {
                                if let Some(text) = value
                                    .pointer("/part/text")
                                    .and_then(serde_json::Value::as_str)
                                {
                                    let _ = tx.send(TransportEvent::Text(text.to_string()));
                                }
                            }
                            Some("error") => {
                                let msg = match value
                                    .pointer("/error/data/message")
                                    .and_then(serde_json::Value::as_str)
                                {
                                    Some(text) => format!("модель: {text}"),
                                    None => "модель вернула ошибку без описания".to_string(),
                                };
                                failed = true;
                                let _ = tx.send(TransportEvent::Failed(msg));
                            }
                            _ => {}
                        }
                    }
                    if !failed {
                        let _ = tx.send(TransportEvent::Finished { session });
                    }
                });
                *child = Some(process);
                Ok(rx)
            }
            Channel::Host { planner, scratch } => {
                // Собирается один раз: повторный `bind` того же пути даёт `EADDRINUSE`, и
                // второй вопрос в TUI упал бы на ровном месте.
                let host = match planner {
                    Some(host) => std::sync::Arc::clone(host),
                    None => {
                        let (tx, rx) = mpsc::channel();
                        let built =
                            crate::host_model::ModelConfig::from_env(HOST_BASE_URL, HOST_MODEL_ID)
                                .map_err(crate::planner_host::PlannerError::Model)
                                .and_then(|config| PlannerHost::prepare(config, scratch, &[]));
                        match built {
                            Ok(host) => {
                                let host = std::sync::Arc::new(host);
                                *planner = Some(std::sync::Arc::clone(&host));
                                host
                            }
                            Err(why) => {
                                // Человек видит причину, TUI остаётся живым.
                                let _ = tx.send(TransportEvent::Failed(why.to_string()));
                                return Ok(rx);
                            }
                        }
                    }
                };
                let (tx, rx) = mpsc::channel();
                let prompt = prompt.to_owned();
                thread::spawn(move || match host.ask(&prompt) {
                    Ok(text) => {
                        let _ = tx.send(TransportEvent::Text(text));
                        let _ = tx.send(TransportEvent::Finished { session: None });
                    }
                    Err(why) => {
                        let _ = tx.send(TransportEvent::Failed(why.to_string()));
                    }
                });
                Ok(rx)
            }
        }
    }

    /// Останавливает запрос: убивает и пожинает процесс. Идемпотентна.
    ///
    /// Для режима `host` отмены нет: поток вправе доработать до `TURN_TIMEOUT` (120 с),
    /// приёмник при этом отбрасывается и поздний ответ никого не обновит. Это записано
    /// здесь, а не выдано за остановку сетевого вызова.
    pub fn cancel(&mut self) {
        if let Channel::Opencode { child, .. } = &mut self.channel {
            if let Some(mut process) = child.take() {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
}

fn write_planner_config(scratch: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(scratch)
        .map_err(|e| format!("create scratch `{}`: {e}", scratch.display()))?;
    let path = scratch.join("stellwerk-planner.json");
    std::fs::write(&path, PLANNER_CONFIG_JSON).map_err(|e| format!("write planner config: {e}"))?;
    Ok(path)
}

/// Снимок репозитория. Отсутствие git — это «нет снимка», а не пустая строка.
fn git_head(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output();
    match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "нет снимка (git rev-parse не удался)".to_string(),
    }
}

/// Состояние экрана плана, которым владеет цикл событий.
struct PlanUi {
    stage: Stage,
    input: String,
    model_text: String,
    error: Option<String>,
    pending: Option<Receiver<TransportEvent>>,
    build: Option<Receiver<Result<BuildOutcome, String>>>,
}

/// Бэкенд для прогона. Токен читается здесь и только здесь: под корнями, которые агент
/// видит из песочницы, его быть не может, иначе прокси обходится чтением файла.
pub fn build_backend_at(token_path: &Path) -> Result<HttpBackend, String> {
    let name = token_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("token");
    let token = read_token_outside(token_path, &build_runner::sandbox_roots(), name)
        .map_err(|why| format!("токен {}: {why}", token_path.display()))?;
    HttpBackend::new(HOST_BASE_URL, HOST_MODEL_ID, TokenProxy::new(token))
        .map_err(|why| format!("бэкенд: {why}"))
}

pub fn build_backend() -> Result<HttpBackend, String> {
    let config = ModelConfig::from_env(HOST_BASE_URL, HOST_MODEL_ID)
        .map_err(|why| format!("конфигурация: {why}"))?;
    build_backend_at(config.token_path())
}

fn pi_program() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("PI_BIN") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or("не задан HOME и не задан PI_BIN")?;
    Ok(PathBuf::from(home).join(".pi/agent/bin/pi"))
}

/// Запуск прогона в отдельном потоке. Бэкенд собирается до потока: «нет токена» — это
/// отказ начать, а не неудачная сборка, и путать их нельзя.
fn start_build(repo: &str, request: BuildRequest, ui: &mut PlanUi) -> Result<(), String> {
    let extension = std::env::var_os("STELLWERK_PI_EXTENSION")
        .map(PathBuf::from)
        .ok_or("не задан STELLWERK_PI_EXTENSION (путь к stellwerk_transport.ts)")?;
    let pi = pi_program()?;
    let backend = build_backend()?;
    let root = std::env::temp_dir().join(format!("stellwerk-build-{}", std::process::id()));
    let repo = PathBuf::from(repo);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let run = BuildRun {
            request: &request,
            repo: &repo,
            root: &root,
            extension: &extension,
            pi: &pi,
        };
        let _ = tx.send(build_runner::run_build(run, &backend));
    });
    ui.build = Some(rx);
    Ok(())
}

fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
}

fn apply_reply(session: &mut PlanSession, reply: Reply, ui: &mut PlanUi) {
    match reply {
        Reply::Question { text } => {
            session.push_turn(Turn::Model(text));
            ui.stage = Stage::Answering;
        }
        Reply::Plan { draft, summary } => {
            session.push_turn(Turn::Model(summary.clone()));
            session.set_draft(draft, summary);
            ui.stage = Stage::Review;
        }
        Reply::Unrecognized { raw } => {
            let repo = PathBuf::from(&session.repo);
            ui.error = Some(match record_unparsed(&repo, &raw) {
                Ok(path) => format!(
                    "модель ответила не по формату; ответ целиком: {}",
                    path.display()
                ),
                // Экран обрезает строки, поэтому без файла улики нет; если и файл не
                // записался, это надо назвать, а не напечатать обрезанный ответ.
                Err(error) => format!("модель ответила не по формату; ответ не сохранён: {error}"),
            });
            ui.stage = Stage::Failed;
        }
    }
}

impl PlanUi {
    fn start(
        &mut self,
        session: &PlanSession,
        transport: &mut LiveTransport,
    ) -> Result<(), String> {
        if self.pending.is_some() {
            return Ok(());
        }
        self.model_text.clear();
        self.error = None;
        let prompt = build_prompt(session);
        let rx = transport.ask(&prompt)?;
        self.pending = Some(rx);
        self.stage = Stage::Asking;
        Ok(())
    }

    /// Итог прогона приходит один раз: поток либо ответил, либо исчез. Исчезновение —
    /// тоже ответ, и он отличается от пустого вывода.
    fn drain_build(&mut self) {
        let Some(rx) = self.build.as_ref() else {
            return;
        };
        let received = match rx.try_recv() {
            Ok(value) => value,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("поток сборки исчез без ответа".to_string()),
        };
        self.build = None;
        match received {
            Ok(outcome) => {
                let text = crate::build_runner::answer_text(&outcome.output);
                let gate = crate::build_runner::gate_summary(&outcome.gate);
                self.error = Some(format!(
                    "Build: ходов модели {} · гейт: {gate} · вывод: {text}",
                    outcome.model_turns
                ));
                self.stage = Stage::Approved;
            }
            Err(why) => {
                self.error = Some(format!("Build завершился ошибкой: {why}"));
                self.stage = Stage::Failed;
            }
        }
    }

    fn drain(
        &mut self,
        session: &mut PlanSession,
        transport: &mut LiveTransport,
        path: &Path,
    ) -> Result<(), String> {
        let Some(rx) = &self.pending else {
            return Ok(());
        };
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(TransportEvent::Text(text)) => self.model_text.push_str(&text),
                Ok(TransportEvent::Failed(message)) => {
                    self.error = Some(message);
                    self.stage = Stage::Failed;
                    finished = true;
                }
                Ok(TransportEvent::Finished { .. }) => finished = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            self.pending = None;
            transport.cancel();
            if self.stage != Stage::Failed {
                let reply = parse_reply(&self.model_text);
                apply_reply(session, reply, self);
            }
            save_session(path, session)?;
        }
        Ok(())
    }
}

/// Точка входа подкоманды `stellwerk plan <repo>`. Держит терминал и цикл
/// событий; отрисовка берётся из чистого `render_plan`.
pub fn run_plan_tui(repo: &Path, requested: PlannerMode) -> Result<(), String> {
    let snapshot = git_head(repo);
    let path = session_path(repo);
    let mut session = load_session(&path)?
        .unwrap_or_else(|| PlanSession::new(&repo.display().to_string(), &snapshot));
    session.reconcile_snapshot(&snapshot);
    let (mode, record) = resolve_mode(session.planner.as_deref(), requested)?;
    if record {
        session.planner = Some(mode.as_str().to_owned());
        save_session(&path, &session)?;
    }
    let scratch = std::env::temp_dir().join(format!("stellwerk-plan-{}", std::process::id()));
    let mut transport = LiveTransport::with_mode(mode, scratch);

    let mut terminal = crate::tui::enter()?;
    let result = plan_loop(&mut terminal, &path, &mut session, &mut transport);
    transport.cancel();
    let restored = crate::tui::leave(&mut terminal);
    result.and(restored)
}

fn initial_stage(session: &PlanSession) -> Stage {
    if session.is_approved() {
        Stage::Approved
    } else if session.draft.is_some() {
        Stage::Review
    } else {
        Stage::Goal
    }
}

fn plan_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    path: &Path,
    session: &mut PlanSession,
    transport: &mut LiveTransport,
) -> Result<(), String> {
    let mut ui = PlanUi {
        stage: initial_stage(session),
        input: String::new(),
        model_text: String::new(),
        error: None,
        pending: None,
        build: None,
    };

    loop {
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(render_plan(&View {
                        session,
                        stage: ui.stage,
                        input: &ui.input,
                        model_text: &ui.model_text,
                        error: ui.error.as_deref(),
                    })),
                    frame.area(),
                );
            })
            .map_err(|e| format!("draw: {e}"))?;

        if event::poll(Duration::from_millis(80)).map_err(|e| format!("poll: {e}"))? {
            if let Event::Key(key) = event::read().map_err(|e| format!("read: {e}"))? {
                if key.kind == KeyEventKind::Press {
                    if is_ctrl_c(&key) {
                        break;
                    }
                    handle_key(&key, &mut ui, session, transport, path)?;
                }
            }
        }

        ui.drain(session, transport, path)?;
        ui.drain_build();
    }
    Ok(())
}

fn handle_key(
    key: &KeyEvent,
    ui: &mut PlanUi,
    session: &mut PlanSession,
    transport: &mut LiveTransport,
    path: &Path,
) -> Result<(), String> {
    match ui.stage {
        Stage::Goal | Stage::Answering => match key.code {
            KeyCode::Enter => {
                let text = std::mem::take(&mut ui.input);
                if !text.trim().is_empty() {
                    if ui.stage == Stage::Goal {
                        session.set_goal(&text);
                    }
                    session.push_turn(Turn::User(text));
                    ui.start(session, transport)?;
                    save_session(path, session)?;
                }
            }
            KeyCode::Backspace => {
                ui.input.pop();
            }
            KeyCode::Char(c) => ui.input.push(c),
            _ => {}
        },
        Stage::Asking => {
            if key.code == KeyCode::Esc {
                transport.cancel();
                ui.pending = None;
                ui.error = None;
                ui.stage = Stage::Cancelled;
            }
        }
        Stage::Review => match key.code {
            KeyCode::Char('a') => {
                session.approve();
                save_session(path, session)?;
                ui.stage = Stage::Approved;
            }
            KeyCode::Char('r') => ui.stage = Stage::Answering,
            _ => {}
        },
        Stage::Approved => {
            if key.code == KeyCode::Char('b') {
                let current = git_head(Path::new(&session.repo));
                match build_request(session, &current) {
                    Err(refusal) => ui.error = Some(format!("Build отклонён: {refusal}")),
                    Ok(request) => match start_build(&session.repo, request, ui) {
                        // Ни «допущен», ни «запущен» не печатается до того, как прогон
                        // действительно начался: бэкенд собирается синхронно и падает здесь.
                        Ok(()) => {
                            ui.error = None;
                            ui.stage = Stage::Building;
                        }
                        Err(why) => ui.error = Some(format!("Build не запущен: {why}")),
                    },
                }
            }
        }
        Stage::Building => {}
        Stage::Failed | Stage::Cancelled => {
            if key.code == KeyCode::Char('r') {
                ui.error = None;
                ui.stage = Stage::Answering;
            }
        }
    }
    Ok(())
}
