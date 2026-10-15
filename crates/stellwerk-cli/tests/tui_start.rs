use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use stellwerk_cli::tui::{render, Phase, State, GATE_AWAITING_HUMAN};

fn test_state() -> State {
    State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Ready,
        verdict: None,
        receipt: None,
        risk: None,
        gate: None,
    }
}

#[test]
fn render_ready_has_enter_hint() {
    let state = test_state();
    let lines = render(&state);

    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("[ START ]"))),
        "render must show [ START ] button"
    );
}

#[test]
fn render_started_shows_fields() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Started,
        verdict: Some("Allow".into()),
        receipt: Some("abc-123".into()),
        risk: Some("R0".into()),
        gate: Some("ждёт человека".into()),
    };

    let lines = render(&state);

    let text: String = lines
        .iter()
        .map(|l| format!("{l}"))
        .collect::<Vec<_>>()
        .join(" ");

    assert!(text.contains("verdict: Allow"), "started must show verdict");
    assert!(
        text.contains("receipt: abc-123"),
        "started must show receipt"
    );
    assert!(text.contains("risk:    R0"), "started must show risk");
    assert!(
        text.contains("gate:    ждёт человека"),
        "started must show gate"
    );
}

#[test]
fn render_started_none_shows_dashes() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Started,
        verdict: Some("Allow".into()),
        receipt: None,
        risk: None,
        gate: Some("ждёт человека".into()),
    };

    let lines = render(&state);

    let text: String = lines
        .iter()
        .map(|l| format!("{l}"))
        .collect::<Vec<_>>()
        .join(" ");

    assert!(text.contains("receipt: —"), "None receipt shows dash");
    assert!(text.contains("risk:    —"), "None risk shows dash");
}

#[test]
fn test_backend_draws() {
    let state = test_state();
    let backend = TestBackend::new(40, 12);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(render(&state)),
                frame.area(),
            );
        })
        .unwrap();

    let buf = terminal.backend_mut().buffer();
    assert!(
        buf.content.iter().any(|c| c.symbol() != ""),
        "buffer must have cells"
    );
}

#[test]
fn real_pi_reason_is_constant() {
    let r = stellwerk_cli::tui::REAL_PI_REASON;
    assert!(
        r.contains("model transport"),
        "reason must mention model transport"
    );
    assert!(r.contains("N2-00"), "reason must reference N2-00");
    assert!(
        r.contains("pre-execution authorization"),
        "reason must mention pre-execution authorization"
    );
}

#[test]
fn gate_constant_is_stable() {
    assert_eq!(GATE_AWAITING_HUMAN, "ждёт человека");
}

#[test]
fn render_deterministic() {
    let state = test_state();
    let a = render(&state);
    let b = render(&state);

    assert_eq!(a.len(), b.len(), "same line count");
    for (la, lb) in a.iter().zip(b.iter()) {
        assert_eq!(
            la.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>(),
            lb.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>(),
            "spans identical"
        );
    }
}
