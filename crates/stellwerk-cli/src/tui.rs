//! TUI-02: состояние экрана, чистый шов рендера и подкоманда `tui`.
//!
//! `render` — единственное место, где `State` становится строками. Функция не
//! знает ни о терминале, ни о цикле событий, ни о crossterm: вход — `&State`,
//! выход — `Vec<Line>`. Поэтому экран проверяется в памяти, без TTY
//! (`tests/tui_render.rs`, `TestBackend`). Всё терминальное живёт в `run_tui`.
//!
//! `START` здесь только показан: запуск исполнения — TUI-03, и он переиспользует
//! существующий путь, а не второй движок.

use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

/// Причина, по которой настоящий Pi показан отключённым, — дословно из TUI-02:
/// включение требует того, чего в срезе ещё нет. Текст обязан совпадать
/// посимвольно; правка строки — это правка требования, а не косметика.
pub const REAL_PI_REASON: &str =
    "требует model transport (N2-00) и pre-execution authorization инициированных моделью инструментов";

/// Метка гейта. Удостоверение фактического запуска есть, решения человека нет —
/// это `ждёт человека`, а не `Done` и не `PASS` (решение 5).
pub const GATE_AWAITING_HUMAN: &str = "ждёт человека";

/// Действие пользователя в цикле событий.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Нажат ENTER — запустить прогон.
    Start,
    /// Нажат `q`/`Esc` — выйти.
    Quit,
}

/// Чем закончился прогон, если он был.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// До `START`: экран показывает действие, исполнения нет.
    Ready,
    /// После `START`: экран показывает результат прогона.
    Started,
}

/// Состояние экрана. Поля результата — `Option`: их отсутствие отличается от
/// пустого значения, иначе «не запускали» было бы неотличимо от «запустили и
/// получили пусто» (закон 6).
#[derive(Debug, Clone)]
pub struct State {
    /// Путь к task-файлу.
    pub task_file: PathBuf,
    /// Путь к агенту (фикстуре) рядом с task-файлом.
    pub agent: PathBuf,
    /// Фаза экрана.
    pub phase: Phase,
    /// Вердикт прогона.
    pub verdict: Option<String>,
    /// id receipt, прочитанный из хранилища.
    pub receipt: Option<String>,
    /// Класс риска из реестра.
    pub risk: Option<String>,
    /// Метка гейта.
    pub gate: Option<String>,
}

/// Чистый рендер: `State` → строки. Ни терминала, ни цикла событий, ни crossterm
/// — только данные. Это и есть шов, из-за которого экран тестируем без TTY.
pub fn render(state: &State) -> Vec<Line<'_>> {
    let mut lines = vec![
        Line::from(Span::styled(
            "Stellwerk — один task-файл",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("task:  {}", state.task_file.display())),
        Line::from(format!("agent: {}", state.agent.display())),
        Line::from(""),
        Line::from(Span::styled(
            "[ START ]",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    if state.phase == Phase::Started {
        lines.push(Line::from(""));
        lines.push(Line::from(format!("verdict: {}", shown(&state.verdict))));
        lines.push(Line::from(format!("receipt: {}", shown(&state.receipt))));
        lines.push(Line::from(format!("risk:    {}", shown(&state.risk))));
        lines.push(Line::from(format!(
            "gate:    {}",
            state.gate.as_deref().unwrap_or(GATE_AWAITING_HUMAN)
        )));
    }

    // Настоящий Pi — видимо отключённая опция: причина видна, выбрать её нельзя.
    // `dim` + `crossed out` отличает строку от доступного действия.
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("[ Real Pi: недоступно ] {REAL_PI_REASON}"),
        Style::default()
            .add_modifier(Modifier::DIM)
            .add_modifier(Modifier::CROSSED_OUT),
    )));

    lines
}

/// Значение поля или явный прочерк: `None` печатается как «—», а не как пустая
/// строка, чтобы «нет данных» нельзя было прочитать как пустое значение.
fn shown(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("—")
}

/// Точка входа подкоманды `stellwerk tui <task.toml>`. Держит терминал и цикл
/// событий — поэтому `render` от них и отделён.
pub fn run_tui(task_file: &Path) -> Result<(), String> {
    let state = State {
        task_file: task_file.to_path_buf(),
        agent: agent_path()?,
        phase: Phase::Ready,
        verdict: None,
        receipt: None,
        risk: None,
        gate: None,
    };

    let mut terminal = enter()?;
    let result = loop_until_quit(&mut terminal, &state);
    let restored = leave(&mut terminal);
    result.and(restored)
}

/// Тот же Pi, что запускает `run`: `PI_BIN`, затем `$HOME/.pi/agent/bin/pi`.
/// Здесь путь только показывается; запуск — по `START`. Отсутствие Pi — названный
/// `MissingEngine`, а не фикстура.
fn agent_path() -> Result<PathBuf, String> {
    crate::pi_program()
}

/// Вход в alternate screen и raw mode. Поломка на полпути откатывает уже
/// сделанное: иначе терминал остался бы в raw mode без экрана.
pub(crate) fn enter() -> Result<Terminal<CrosstermBackend<Stdout>>, String> {
    enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| {
        let _ = disable_raw_mode();
        format!("enter alternate screen: {e}")
    })?;
    Terminal::new(CrosstermBackend::new(stdout)).map_err(|e| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        format!("terminal: {e}")
    })
}

/// Выход из raw mode и alternate screen. Возврат терминала — отдельный результат:
/// поломка выхода обязана быть видна, а не потеряться вместе с успехом цикла.
pub(crate) fn leave(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), String> {
    disable_raw_mode().map_err(|e| format!("disable raw mode: {e}"))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| format!("leave alternate screen: {e}"))
}

/// Цикл событий: рисуем состояние и ждём действий (`Enter` / `q`/`Esc`).
/// Enter запускает прогон существующего пути через `run_task`.
fn loop_until_quit(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &State,
) -> Result<(), String> {
    let state = std::sync::Arc::new(std::sync::Mutex::new(state.clone()));
    let task_file = state.lock().unwrap().task_file.clone();
    loop {
        let guard = state.lock().unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(Paragraph::new(render(&guard)), area);
            })
            .map_err(|e| format!("draw: {e}"))?;
        drop(guard);

        // Таймаут, а не блокирующее чтение: экран переживает отсутствие нажатий и
        // не зависает насмерть в ожидании ввода.
        if !event::poll(Duration::from_millis(250)).map_err(|e| format!("poll: {e}"))? {
            continue;
        }
        if let Event::Key(key) = event::read().map_err(|e| format!("read: {e}"))? {
            if key.kind == KeyEventKind::Press {
                if key.code == KeyCode::Enter {
                    let arc = state.clone();
                    return loop_handle_enter(task_file.clone(), arc);
                }
                if is_quit(&key) {
                    return Ok(());
                }
            }
        }
    }
}

/// Обработка нажатия Enter: запускает `run_task` (тот же путь, что и `Commands::Run`),
/// читает `RunResult` и обновляет state.
fn loop_handle_enter(
    task_file: PathBuf,
    state: std::sync::Arc<std::sync::Mutex<State>>,
) -> Result<(), String> {
    let result = crate::run_task(&task_file)?;
    let mut guard = state.lock().unwrap();
    guard.phase = Phase::Started;
    guard.verdict = Some(result.verdict);
    guard.receipt = Some(result.receipt_id);
    guard.risk = Some(result.risk);
    guard.gate = Some(result.gate);
    Ok(())
}

/// `q` или `Esc` завершают показ. Отпускания и повторы нажатием не считаются:
/// реакция на `Release` выглядела бы как двойное срабатывание.
fn is_quit(key: &KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
}
