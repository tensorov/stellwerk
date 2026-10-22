//! Tests for the Plan surface. All of them are pure: no terminal, no network, no
//! live model. The one property that needs the real model — that the planner has
//! no tools — is checked here structurally against the generated config, and is
//! exercised live in `docs/demos/plan-build.md`.

use std::path::PathBuf;

use stellwerk_cli::plan::{
    build_backend_at, build_prompt, build_request, load_session, parse_reply, render_plan,
    resolve_mode, save_session, session_path, BuildRefusal, PlanDraft, PlanSession, PlannerMode,
    Reply, Stage, Turn, View, PLANNER_CONFIG_JSON, PLANNER_MODEL,
};

fn draft(goal: &str) -> PlanDraft {
    PlanDraft {
        goal: goal.to_string(),
        allowed_changes: vec!["src/lib.rs".to_string()],
        acceptance: vec!["тест проходит".to_string()],
        checks: vec!["cargo test".to_string()],
        constraints: vec!["без новых зависимостей".to_string()],
    }
}

/// Утверждённый план, сделанный нашим транспортом на снимке `abc`: исходное состояние для
/// проверок допуска к исполнению.
fn approved_host_plan() -> PlanSession {
    let mut session = PlanSession::new("/repo", "abc");
    session.set_goal("цель");
    session.set_draft(draft("цель"), "сводка".to_string());
    session.approve();
    session.planner = Some("host".to_string());
    session
}

fn tmp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("stellwerk-plan-test-{name}-{nanos}"))
}

#[test]
fn a_question_reply_is_recognized() {
    let reply = parse_reply(r#"{"kind":"question","text":"Какой репозиторий?"}"#);
    match reply {
        Reply::Question { text } => assert_eq!(text, "Какой репозиторий?"),
        other => panic!("ожидался вопрос, получено: {other:?}"),
    }
}

#[test]
fn a_plan_reply_is_recognized() {
    let raw = r#"{"kind":"plan","summary":"кратко",
        "goal":"исправить опечатку",
        "allowed_changes":["src/lib.rs"],
        "acceptance":["тест зелёный"],
        "checks":["cargo test"],
        "constraints":["без новых зависимостей"]}"#;
    match parse_reply(raw) {
        Reply::Plan { draft, summary } => {
            assert_eq!(draft.goal, "исправить опечатку");
            assert_eq!(draft.allowed_changes, vec!["src/lib.rs".to_string()]);
            assert_eq!(summary, "кратко");
        }
        other => panic!("ожидался план, получено: {other:?}"),
    }
}

#[test]
fn a_plan_wrapped_in_prose_is_still_extracted() {
    let raw = "Вот план:\n{\"kind\":\"plan\",\"summary\":\"s\",\"goal\":\"g\",\
               \"allowed_changes\":[],\"acceptance\":[],\"checks\":[],\"constraints\":[]}\nГотово.";
    match parse_reply(raw) {
        Reply::Plan { draft, .. } => assert_eq!(draft.goal, "g"),
        other => panic!("ожидался план из прозы, получено: {other:?}"),
    }
}

#[test]
fn an_unparseable_reply_is_a_distinct_state_not_a_default() {
    match parse_reply("я не понял задачу") {
        Reply::Unrecognized { raw } => assert_eq!(raw, "я не понял задачу"),
        other => panic!("неразобранное обязано быть Unrecognized, получено: {other:?}"),
    }
}

#[test]
fn approving_marks_the_current_version() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_goal("исправить опечатку");
    s.set_draft(draft("исправить опечатку"), "кратко".to_string());
    assert!(s.approval_required(), "до утверждения нужен человек");
    s.approve();
    assert!(!s.approval_required(), "утверждённая версия одобрена");
    assert!(s.is_approved());
}

#[test]
fn a_changed_draft_after_approval_requires_a_new_approval() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_goal("исправить опечатку");
    s.set_draft(draft("исправить опечатку"), "кратко".to_string());
    s.approve();
    s.set_draft(draft("исправить опечатку"), "иная версия".to_string());
    assert!(
        s.approval_required(),
        "изменённый план обязан требовать нового утверждения"
    );
    assert!(!s.is_approved());
}

#[test]
fn changing_plan_text_without_bumping_the_version_still_requires_approval() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_draft(draft("цель"), "кратко".to_string());
    s.approve();
    assert!(
        !s.approval_required(),
        "сразу после утверждения человек не нужен"
    );
    // Правка текста в обход `set_draft`: поля `draft` публичны, счётчик версии
    // не двигается — поймать обязан именно дайджест текста, а не версия.
    s.draft.as_mut().unwrap().goal = "другая цель".to_string();
    assert_eq!(s.version, 1, "версия намеренно не менялась");
    assert!(
        s.approval_required(),
        "изменённый текст плана обязан требовать нового утверждения"
    );
    assert!(!s.is_approved());
}

#[test]
fn a_draft_has_a_text_digest() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.draft = Some(draft("цель"));
    assert!(
        s.plan_digest().is_some(),
        "у черновика обязан быть дайджест текста"
    );
}

#[test]
fn the_digest_is_stable_for_identical_text() {
    let mut a = PlanSession::new("/repo", "head-a");
    a.set_draft(draft("один и тот же план"), "s".to_string());
    let mut b = PlanSession::new("/repo", "head-b");
    b.set_draft(draft("один и тот же план"), "s".to_string());
    assert!(a.plan_digest().is_some(), "дайджест существует");
    assert_eq!(
        a.plan_digest(),
        b.plan_digest(),
        "один и тот же текст даёт один и тот же дайджест, независимо от снимка"
    );
}

#[test]
fn absent_summary_and_empty_summary_are_different_texts() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.draft = Some(draft("цель"));
    let absent = s.plan_digest();
    s.summary = Some(String::new());
    assert_ne!(
        s.plan_digest(),
        absent,
        "отсутствие summary и пустое summary — разные состояния (закон 6)"
    );
}

#[test]
fn a_model_claim_of_approval_does_not_approve_anything() {
    // Ответ модели «план утверждён» — это текст, а не действие TUI. Утверждение
    // существует только как `approve()` от нажатия человека.
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_draft(draft("g"), "s".to_string());
    s.push_turn(Turn::Model("план утверждён".to_string()));
    assert!(s.approval_required(), "текст модели не утверждает план");
}

#[test]
fn a_session_round_trips_through_disk() {
    let dir = tmp("roundtrip");
    let path = session_path(&dir);
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_goal("цель");
    s.set_draft(draft("цель"), "кратко".to_string());
    s.approve();
    save_session(&path, &s).unwrap();

    let loaded = load_session(&path).unwrap().expect("сессия сохранена");
    assert_eq!(loaded.goal.as_deref(), Some("цель"));
    assert_eq!(loaded.version, s.version);
    assert_eq!(loaded.approved_version, s.approved_version);
    assert_eq!(loaded.approved_digest, s.approved_digest);
    assert_eq!(loaded.plan_digest(), s.plan_digest());
    assert!(loaded.is_approved());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_session_is_absent_not_a_default() {
    let dir = tmp("missing");
    let path = session_path(&dir);
    assert!(
        load_session(&path).unwrap().is_none(),
        "отсутствие файла — отдельное состояние, а не пустая сессия"
    );
}

#[test]
fn render_shows_goal_input_before_anything_else() {
    let s = PlanSession::new("/repo", "abc123");
    let text = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Goal,
        input: "добавить тест",
        model_text: "",
        error: None,
    }));
    assert!(text.contains("цель"), "на старте видно поле цели: {text}");
    assert!(text.contains("добавить тест"), "виден ввод: {text}");
    assert!(
        text.contains("сначала утвердите план"),
        "до утверждения Build не обещает запуск: {text}"
    );
    assert!(
        !text.contains("недоступен"),
        "после N2-00h3d «недоступен» — неправда: {text}"
    );
}

#[test]
fn render_shows_draft_fields_and_pending_approval() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_goal("исправить опечатку");
    s.set_draft(draft("исправить опечатку"), "кратко".to_string());
    let text = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Review,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(
        text.contains("исправить опечатку"),
        "видна цель плана: {text}"
    );
    assert!(text.contains("src/lib.rs"), "виден допустимый файл: {text}");
    assert!(
        text.contains("ждёт утверждения"),
        "неутверждённый план ждёт человека: {text}"
    );
    assert!(
        text.contains("сначала утвердите план"),
        "неутверждённый план не запускает Build: {text}"
    );
}

#[test]
fn render_shows_approval_once_the_human_approved() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.set_draft(draft("g"), "s".to_string());
    s.approve();
    let text = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Approved,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(text.contains("утвержд"), "виден факт утверждения: {text}");
    assert!(
        text.contains("b — запустить"),
        "утверждённый план называет действие: {text}"
    );
}

#[test]
fn planner_config_grants_no_mutating_execution_or_network_tool() {
    // Строгая проверка структуры, а не поиск подстрок: любой инструмент кроме
    // scoped `read` обязан быть ровно `false`, поэтому новый включённый
    // инструмент не проскочит молча.
    let config: serde_json::Value = serde_json::from_str(PLANNER_CONFIG_JSON).unwrap();
    let tools = config
        .pointer("/agent/stellwerk-planner/tools")
        .and_then(serde_json::Value::as_object)
        .expect("конфиг планировщика содержит объект tools");
    for (name, enabled) in tools {
        let expected = name == "read";
        assert_eq!(
            enabled.as_bool(),
            Some(expected),
            "инструмент `{name}` обязан быть {}",
            if expected {
                "включён"
            } else {
                "выключен"
            }
        );
    }
    for required in ["bash", "edit", "write", "patch", "task", "webfetch"] {
        assert!(
            tools.get(required).is_some(),
            "инструмент `{required}` обязан присутствовать и быть выключенным"
        );
    }
    assert_eq!(
        config
            .pointer("/agent/stellwerk-planner/model")
            .and_then(serde_json::Value::as_str),
        Some(PLANNER_MODEL),
        "модель планировщика пинована явно"
    );
}

#[test]
fn a_snapshot_drift_after_approval_forces_reapproval() {
    let mut s = PlanSession::new("/repo", "old-head");
    s.set_draft(draft("g"), "s".to_string());
    s.approve();
    assert!(!s.approval_required(), "до сдвига план утверждён");
    s.reconcile_snapshot("new-head");
    assert_eq!(s.snapshot, "new-head");
    assert_eq!(s.version, 2, "смена дерева — новая версия, а не прежняя");
    assert!(
        s.approval_required(),
        "другое дерево — прежнее утверждение недействительно"
    );
    assert!(!s.is_approved());
}

#[test]
fn a_matching_snapshot_leaves_approval_intact() {
    let mut s = PlanSession::new("/repo", "same-head");
    s.set_draft(draft("g"), "s".to_string());
    s.approve();
    s.reconcile_snapshot("same-head");
    assert!(
        !s.approval_required(),
        "тот же снимок не сбрасывает утверждение"
    );
}

#[test]
fn set_goal_and_set_draft_bump_the_version() {
    let mut s = PlanSession::new("/repo", "abc123");
    assert_eq!(s.version, 0);
    s.set_goal("цель");
    assert_eq!(s.version, 1, "новая цель — новая версия");
    s.set_draft(draft("цель"), "кратко".to_string());
    assert_eq!(s.version, 2, "новый черновик — новая версия");
}

#[test]
fn extract_json_rejects_reversed_braces() {
    match parse_reply("}\n{") {
        Reply::Unrecognized { raw } => assert_eq!(raw, "}\n{"),
        other => panic!("перевёрнутые скобки обязаны быть Unrecognized: {other:?}"),
    }
}

#[test]
fn an_unreadable_path_is_an_error_not_an_absent_session() {
    let dir = tmp("dir-as-session");
    std::fs::create_dir_all(&dir).unwrap();
    let result = load_session(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        result.is_err(),
        "каталог вместо файла — ошибка чтения, а не «сессии нет»"
    );
}

#[test]
fn render_labels_the_current_stage_and_the_answering_prompt() {
    let s = PlanSession::new("/repo", "abc123");
    let goal = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Goal,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(goal.contains("готов"), "стартовая фаза подписана: {goal}");
    let asking = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Asking,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(
        asking.contains("отвечает модель"),
        "фаза ответа подписана: {asking}"
    );
    assert!(
        asking.contains("модель отвечает"),
        "виден ход ответа: {asking}"
    );
    let answering = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Answering,
        input: "уточнить",
        model_text: "",
        error: None,
    }));
    assert!(
        answering.contains("ответ> уточнить"),
        "виден ввод ответа: {answering}"
    );
}

#[test]
fn an_empty_session_render_has_no_dialogue_or_live_text() {
    let s = PlanSession::new("/repo", "abc123");
    let text = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Goal,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(
        !text.contains("диалог:"),
        "без реплик нет секции диалога: {text}"
    );
    assert!(
        !text.contains("модель (сейчас)"),
        "без текста модели нет строки: {text}"
    );
}

#[test]
fn build_prompt_carries_goal_snapshot_and_json_contract() {
    let mut s = PlanSession::new("/repo", "deadbeef");
    s.set_goal("исправить опечатку");
    let prompt = build_prompt(&s);
    assert!(
        prompt.contains("исправить опечатку"),
        "цель в промпте: {prompt}"
    );
    assert!(prompt.contains("/repo"), "репозиторий в промпте: {prompt}");
    assert!(prompt.contains("deadbeef"), "снимок в промпте: {prompt}");
    assert!(
        prompt.contains("\"kind\":\"plan\""),
        "контракт JSON объявлен: {prompt}"
    );
    assert!(
        prompt.contains("НЕ утверждаешь"),
        "запрет самоутверждения объявлен: {prompt}"
    );
}

fn flatten(lines: &[ratatui::text::Line<'static>]) -> String {
    lines
        .iter()
        .map(|l| format!("{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// N2-00h2b: отсутствующая запись заполняется текущим режимом и остаётся в файле.
#[test]
fn the_session_records_the_planner_mode() {
    let (mode, record) = resolve_mode(None, PlannerMode::Host).expect("режим разрешён");
    assert_eq!(mode, PlannerMode::Host);
    assert!(record, "запись отсутствует — её и создаём");

    let mut s = PlanSession::new("/repo", "abc123");
    s.planner = Some(mode.as_str().to_string());
    let path = tmp("planner-record");
    save_session(&path, &s).expect("сессия сохранена");
    let loaded = load_session(&path).expect("чтение").expect("сессия есть");
    assert_eq!(loaded.planner.as_deref(), Some("host"));
}

/// Записанный режим старше флага: иначе утверждённый план менял бы происхождение при
/// перезапуске, и Build нельзя было бы спросить, как этот план получен.
#[test]
fn a_recorded_mode_is_not_overwritten_by_a_different_flag() {
    let (mode, record) = resolve_mode(Some("host"), PlannerMode::Opencode).expect("режим разрешён");
    assert_eq!(mode, PlannerMode::Host);
    assert!(!record, "записанное не перетирается флагом");

    assert!(
        resolve_mode(Some("турбо"), PlannerMode::Host).is_err(),
        "неизвестная запись — отказ, а не host"
    );
}

/// Старая сессия без поля читается как «происхождение не записано», а не как `host`.
#[test]
fn an_old_session_without_the_field_is_not_treated_as_host() {
    let path = tmp("planner-old");
    std::fs::write(
        &path,
        r#"{"repo":"/repo","snapshot":"abc","goal":null,"turns":[],"draft":null,"summary":null,"version":0,"approved_version":null}"#,
    )
    .expect("старая сессия записана");
    let loaded = load_session(&path).expect("чтение").expect("сессия есть");
    assert_eq!(loaded.planner, None, "нет записи — не host");
    let (mode, record) =
        resolve_mode(loaded.planner.as_deref(), PlannerMode::Host).expect("режим разрешён");
    assert_eq!(mode, PlannerMode::Host);
    assert!(record, "после загрузки запись создаётся");
}

/// Режим виден человеку, и `opencode` назван тем, чем он является — ходом вне транспорта.
#[test]
fn render_names_the_mode_and_calls_opencode_external() {
    let mut s = PlanSession::new("/repo", "abc123");
    s.planner = Some("opencode".to_string());
    let text = flatten(&render_plan(&View {
        session: &s,
        stage: Stage::Goal,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(text.contains("opencode"), "режим виден: {text}");
    assert!(text.contains("вне транспорта"), "пометка есть: {text}");

    let fresh = PlanSession::new("/repo", "abc123");
    let text = flatten(&render_plan(&View {
        session: &fresh,
        stage: Stage::Goal,
        input: "",
        model_text: "",
        error: None,
    }));
    assert!(
        text.contains("не записан"),
        "нет записи — так и сказано: {text}"
    );
}

/// N2-00h3a: план без утверждения не исполняется — и причина названа версией, а не «нет».
#[test]
fn build_requires_an_approved_plan() {
    let empty = PlanSession::new("/repo", "abc");
    assert_eq!(build_request(&empty, "abc"), Err(BuildRefusal::NoPlan));

    let mut pending = PlanSession::new("/repo", "abc");
    pending.set_goal("цель");
    pending.set_draft(draft("цель"), "сводка".to_string());
    assert_eq!(
        build_request(&pending, "abc"),
        Err(BuildRefusal::Unapproved {
            version: 2,
            approved: None
        })
    );
}

/// Правка после утверждения снова требует человека: версия выросла, утверждение отстало.
#[test]
fn an_edit_after_approval_invalidates_the_request() {
    let mut s = approved_host_plan();
    s.set_draft(draft("другая цель"), "сводка".to_string());
    assert_eq!(
        build_request(&s, "abc"),
        Err(BuildRefusal::Unapproved {
            version: 3,
            approved: Some(2)
        })
    );
}

/// Текст правили в обход счётчика версий — дайджест это ловит отдельно от `version`.
#[test]
fn a_text_edit_without_a_version_bump_is_caught_by_the_digest() {
    let mut s = approved_host_plan();
    s.draft
        .as_mut()
        .expect("черновик")
        .checks
        .push("cargo clippy".to_string());
    assert_eq!(
        build_request(&s, "abc"),
        Err(BuildRefusal::DigestChanged { version: 2 })
    );
}

/// План утверждён для одного состояния дерева; исполнять его на другом — не «мелочь».
#[test]
fn a_plan_approved_for_another_snapshot_is_refused() {
    let s = approved_host_plan();
    let refusal = build_request(&s, "def").expect_err("снимок разошёлся");
    match &refusal {
        BuildRefusal::SnapshotDrift { planned, current } => {
            assert_eq!((planned.as_str(), current.as_str()), ("abc", "def"));
        }
        other => panic!("ожидался SnapshotDrift, получено: {other:?}"),
    }
    let text = refusal.to_string();
    assert!(
        text.contains("abc") && text.contains("def"),
        "оба снимка видны: {text}"
    );
}

/// Нет записи о происхождении — это не «сделан вне транспорта» и не «host».
#[test]
fn an_unrecorded_planner_is_not_host() {
    let mut s = approved_host_plan();
    s.planner = None;
    assert_eq!(
        build_request(&s, "abc"),
        Err(BuildRefusal::PlannerNotRecorded)
    );
    assert_ne!(
        build_request(&s, "abc"),
        Err(BuildRefusal::PlannerBypassedTransport {
            recorded: "opencode".to_string()
        }),
        "неизвестное не превращается в известный отказ"
    );
}

/// План, сделанный мимо транспорта, отвергается; незнакомая запись названа своей.
#[test]
fn opencode_is_named_as_bypassing_the_transport() {
    let mut bypassed = approved_host_plan();
    bypassed.planner = Some("opencode".to_string());
    let refusal = build_request(&bypassed, "abc").expect_err("opencode не пропускается");
    assert_eq!(
        refusal,
        BuildRefusal::PlannerBypassedTransport {
            recorded: "opencode".to_string()
        }
    );
    assert!(refusal.to_string().contains("opencode"), "{refusal}");

    let mut odd = approved_host_plan();
    odd.planner = Some("турбо".to_string());
    assert_eq!(
        build_request(&odd, "abc"),
        Err(BuildRefusal::PlannerUnknown {
            recorded: "турбо".to_string()
        })
    );
}

/// Утверждённый план с записанным `host` и тем же снимком даёт запрос с тем же содержимым.
#[test]
fn an_approved_host_plan_becomes_a_request() {
    let request = build_request(&approved_host_plan(), "abc").expect("план разрешён");
    assert_eq!(request.goal, "цель");
    assert_eq!(request.allowed_changes, vec!["src/lib.rs".to_string()]);
    assert_eq!(request.checks, vec!["cargo test".to_string()]);
    assert_eq!(request.acceptance, vec!["тест проходит".to_string()]);
}

/// Отсутствие токена — отдельное состояние, а не «бэкенд, который потом как-нибудь».
/// Прогон не начинается, и в причине назван именно тот файл, которого нет.
#[test]
fn the_build_names_the_missing_token() {
    let missing = tmp("token").join("neuraldeep.token");
    let why = match build_backend_at(&missing) {
        Ok(_) => panic!("токена нет, а бэкенд собрался"),
        Err(why) => why,
    };
    assert!(why.contains("neuraldeep.token"), "{why}");
    assert!(why.contains(&missing.display().to_string()), "{why}");
}

/// Отказ разбора обязан оставить улику: экран обрезает ответ, и без файла причина
/// «не по формату» непроверяема.
#[test]
fn an_unparsed_reply_is_written_down() {
    let dir = std::env::temp_dir().join(format!(
        "sw-unparsed-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("время")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("каталог");
    let raw = "{\"kind\":\"plan\",\"summary\":\"обрезано";
    let path = stellwerk_cli::plan::record_unparsed(&dir, raw).expect("ответ записан");
    assert!(
        path.starts_with(&dir),
        "путь внутри репозитория: {}",
        path.display()
    );
    assert_eq!(std::fs::read_to_string(&path).expect("файл"), raw);
}

/// Модель иногда описывает правку объектом. Путь в нём есть, и он же — скоуп; отказ
/// разбора здесь был бы отказом из-за формы, а не из-за содержания.
#[test]
fn a_plan_with_object_entries_names_the_file() {
    let raw = "{\"kind\":\"plan\",\"summary\":\"две\",\"goal\":\"две\",\
        \"allowed_changes\":[{\"file\":\"src/lib.rs\",\"change\":\"добавить two\"}],\
        \"acceptance\":[],\"checks\":[],\"constraints\":[]}";
    let Reply::Plan { draft, .. } = parse_reply(raw) else {
        panic!("план не разобран: {raw}");
    };
    assert_eq!(draft.allowed_changes, vec!["src/lib.rs".to_string()]);
}

/// А вот объект без пути — это не план с пустым скоупом, а неразобранный ответ.
#[test]
fn a_scope_entry_without_a_file_is_not_guessed() {
    let raw = "{\"kind\":\"plan\",\"summary\":\"две\",\"goal\":\"две\",\
        \"allowed_changes\":[{\"change\":\"добавить two\"}],\
        \"acceptance\":[],\"checks\":[],\"constraints\":[]}";
    assert!(matches!(parse_reply(raw), Reply::Unrecognized { .. }));
}
