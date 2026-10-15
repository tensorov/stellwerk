use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use stellwerk_cli::tui::{render, Phase, State, GATE_AWAITING_HUMAN, REAL_PI_REASON};

#[test]
fn render_ready_screen() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Ready,
        verdict: None,
        receipt: None,
        risk: None,
        gate: None,
    };

    let lines = render(&state);

    assert!(
        lines[0]
            .spans
            .iter()
            .any(|s| s.content.contains("Stellwerk")),
        "header must contain Stellwerk"
    );
    assert!(
        lines[1].spans.iter().any(|s| s.content.contains("task:"))
            && lines[1]
                .spans
                .iter()
                .any(|s| s.content.contains("task.toml")),
        "must show task path"
    );
    assert!(
        lines[2].spans.iter().any(|s| s.content.contains("agent:"))
            && lines[2]
                .spans
                .iter()
                .any(|s| s.content.contains("pi-fixture.sh")),
        "must show agent path"
    );
    assert!(
        lines[4]
            .spans
            .iter()
            .any(|s| s.content.contains("[ START ]")),
        "must show START"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("verdict:"))),
        "phase=Ready must not show verdict"
    );
}

#[test]
fn render_started_screen() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Started,
        verdict: Some("Allow".to_string()),
        receipt: Some("abc-123".to_string()),
        risk: Some("R0".to_string()),
        gate: Some("ждёт человека".to_string()),
    };

    let lines = render(&state);

    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("verdict: Allow"))),
        "must show verdict"
    );
    assert!(
        lines.iter().any(|l| l
            .spans
            .iter()
            .any(|s| s.content.contains("receipt: abc-123"))),
        "must show receipt"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("risk:    R0"))),
        "must show risk"
    );
    assert!(
        lines.iter().any(|l| l
            .spans
            .iter()
            .any(|s| s.content.contains("gate:    ждёт человека"))),
        "must show gate"
    );
}

#[test]
fn render_started_with_none_shows_dashes() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Started,
        verdict: Some("Allow".to_string()),
        receipt: None,
        risk: None,
        gate: Some("ждёт человека".to_string()),
    };

    let lines = render(&state);

    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("receipt: —"))),
        "None receipt must show dash"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("risk:    —"))),
        "None risk must show dash"
    );
}

#[test]
fn test_backend_integration() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Started,
        verdict: Some("Allow".to_string()),
        receipt: Some("abc-123".to_string()),
        risk: Some("R0".to_string()),
        gate: Some("ждёт человека".to_string()),
    };

    let backend = TestBackend::new(40, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new(render(&state)), frame.area());
        })
        .unwrap();

    let buffer = terminal.backend_mut().buffer();
    assert!(
        buffer.content.iter().any(|cell| !cell.symbol().is_empty()),
        "buffer must have cells"
    );
}

#[test]
fn real_pi_reason_is_stable() {
    assert!(
        REAL_PI_REASON.contains("model transport"),
        "must mention model transport"
    );
    assert!(REAL_PI_REASON.contains("N2-00"), "must reference N2-00");
    assert!(
        REAL_PI_REASON.contains("pre-execution authorization"),
        "must mention pre-execution authorization"
    );
}

#[test]
fn gate_awaiting_human_is_stable() {
    assert_eq!(GATE_AWAITING_HUMAN, "ждёт человека");
}

#[test]
fn render_is_pure() {
    let state = State {
        task_file: PathBuf::from("task.toml"),
        agent: PathBuf::from("pi-fixture.sh"),
        phase: Phase::Ready,
        verdict: None,
        receipt: None,
        risk: None,
        gate: None,
    };

    let lines1 = render(&state);
    let lines2 = render(&state);

    assert_eq!(lines1.len(), lines2.len(), "render must be deterministic");
    for (l1, l2) in lines1.iter().zip(lines2.iter()) {
        assert_eq!(l1.spans.len(), l2.spans.len(), "spans count must match");
        for (s1, s2) in l1.spans.iter().zip(l2.spans.iter()) {
            assert_eq!(
                s1.content.as_ref(),
                s2.content.as_ref(),
                "spans content must match"
            );
        }
    }
}
