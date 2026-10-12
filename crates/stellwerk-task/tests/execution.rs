//! Спецификация N1-02: `ExecutionSpec`, реестр профилей и отказ ДО запуска.
//!
//! Проверяются пять классов adversarial:
//! 1. `malformed_input` — отсутствие спецификации, пустая программа и неизвестный движок
//!    дают КАЖДЫЙ свой отказ, а не один общий;
//! 2. `missing_data_is_a_state` — «не настроено», «настроено с пустой программой» и
//!    «настроено и разрешено» — три различимых исхода;
//! 3. `stale_state` — выбранный профиль захвачен при разрешении и не пересчитывается
//!    при чтении;
//! 4. `dirty_worktree` — проверяется извне, `git status` (не здесь);
//! 5. `misleading_success_output` — реестр, в котором ничего нет, отказывает всему.
//!
//! Плюс доказательство «отказ до запуска»: программа, которая при запуске оставила бы
//! файл-маркер, не запускается никогда.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use stellwerk_task::execution::{
    resolve, EngineProfile, EngineRegistry, ExecutionRefusal, ExecutionSpec, RunExecution, REGISTRY,
};

/// Два реестра под один движок, но с разными аргументами: на них проверяется, что профиль
/// захвачен при разрешении, а не перечитан по действующему реестру.
const PROFILES_A: &[EngineProfile] = &[EngineProfile::new("pi", &["--from-a"])];
const PROFILES_B: &[EngineProfile] = &[EngineProfile::new("pi", &["--from-b"])];

fn spec(program: &str, engine: &str) -> ExecutionSpec {
    ExecutionSpec {
        program: program.to_owned(),
        engine: engine.to_owned(),
    }
}

fn unique(prefix: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("часы после эпохи")
        .as_nanos();
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("stellwerk-n1-02-{prefix}-{nanos}-{n}"))
}

// ── 1. malformed_input: три причины, три собственных отказа ──────────────────────────

#[test]
fn an_absent_spec_is_refused_and_named_as_absent() {
    let refusal = resolve(None, &REGISTRY).expect_err("нет спецификации — отказ");
    assert!(matches!(refusal, ExecutionRefusal::Absent), "{refusal:?}");
    assert_ne!(refusal, ExecutionRefusal::EmptyProgram);
}

#[test]
fn a_blank_program_is_its_own_refusal_not_an_absent_spec() {
    for blank in ["", "   ", "\t"] {
        let refusal =
            resolve(Some(&spec(blank, "pi")), &REGISTRY).expect_err("пустая программа — отказ");
        assert!(
            matches!(refusal, ExecutionRefusal::EmptyProgram),
            "программа {blank:?} — пустая, а не {refusal:?}"
        );
        assert_ne!(refusal, ExecutionRefusal::Absent);
    }
}

#[test]
fn an_unknown_engine_is_refused_with_its_own_name() {
    let refusal = resolve(
        Some(&spec("/bin/true", "definitely-not-registered")),
        &REGISTRY,
    )
    .expect_err("движок вне реестра — отказ");
    match refusal {
        ExecutionRefusal::UnknownProfile { engine, known } => {
            assert_eq!(engine, "definitely-not-registered", "виновник назван");
            assert!(
                known.contains(&"pi".to_owned()),
                "отказ называет, что было разрешено: {known:?}"
            );
        }
        other => panic!("неизвестный движок — свой отказ, а не {other:?}"),
    }
}

// ── 2. missing_data_is_a_state: три различимых исхода ─────────────────────────────────

#[test]
fn absent_blank_and_allowed_are_three_distinct_outcomes() {
    let absent = resolve(None, &REGISTRY);
    let blank = resolve(Some(&spec("", "pi")), &REGISTRY);
    let allowed = resolve(Some(&spec("/bin/true", "pi")), &REGISTRY);

    assert!(matches!(absent, Err(ExecutionRefusal::Absent)));
    assert!(matches!(blank, Err(ExecutionRefusal::EmptyProgram)));
    assert!(allowed.is_ok(), "разрешённый движок обязан разрешиться");

    // Пары исходов не равны: «нет данных» не выдаётся за «пусто» и наоборот.
    assert_ne!(absent, blank);
    assert_ne!(absent, allowed);
    assert_ne!(blank, allowed);
}

#[test]
fn absent_and_blank_differ_even_though_program_is_missing_in_both_eyes() {
    // «Спецификации нет» и «спецификация есть, но пуста» — не одно и то же состояние.
    let absent = resolve(None, &REGISTRY);
    let blank = resolve(Some(&spec(" \n ", "pi")), &REGISTRY);
    assert!(!matches!(absent, Err(ExecutionRefusal::EmptyProgram)));
    assert!(!matches!(blank, Err(ExecutionRefusal::Absent)));
}

// ── 3. stale_state: профиль захвачен, а не пересчитан при чтении ──────────────────────

#[test]
fn the_selected_profile_is_captured_and_not_recomputed_on_read() {
    let earlier = EngineRegistry::of(PROFILES_A);
    let first = resolve(Some(&spec("/bin/true", "pi")), &earlier).expect("pi разрешён");
    let entry = first.run_entry();
    assert_eq!(entry.args, ["--from-a"], "снимок взял профиль-А");

    // Реестр, действующий ПОЗЖЕ, даёт другой профиль...
    let later = EngineRegistry::of(PROFILES_B);
    let second = resolve(Some(&spec("/bin/true", "pi")), &later).expect("pi разрешён и там");
    assert_eq!(second.profile().args(), ["--from-b"]);

    // ...но уже сделанный выбор не пересчитывается по нему.
    assert_eq!(first.profile().args(), ["--from-a"]);
    assert_eq!(first.run_entry(), entry);
    assert_eq!(
        entry,
        RunExecution {
            program: "/bin/true".to_owned(),
            engine: "pi".to_owned(),
            args: vec!["--from-a".to_owned()],
        }
    );
}

#[test]
fn a_resolved_profile_carries_program_engine_and_arguments() {
    let resolved = resolve(
        Some(&spec("/bin/true", "pi")),
        &EngineRegistry::of(PROFILES_A),
    )
    .expect("pi");
    assert_eq!(resolved.program(), "/bin/true");
    assert_eq!(resolved.engine(), "pi");
    assert_eq!(resolved.spec().program, "/bin/true");
    assert_eq!(resolved.profile().args(), ["--from-a"]);
}

// ── 5. misleading_success_output: реестр, принимающий всё, — не реестр ─────────────────

#[test]
fn a_registry_that_holds_nothing_refuses_every_engine() {
    let empty = EngineRegistry::of(&[]);
    let refusal =
        resolve(Some(&spec("/bin/true", "pi")), &empty).expect_err("пустой реестр обязан отказать");
    match refusal {
        ExecutionRefusal::UnknownProfile { engine, known } => {
            assert_eq!(engine, "pi");
            assert!(known.is_empty(), "разрешать было нечего: {known:?}");
        }
        other => panic!("пустой реестр — отказ, а не {other:?}"),
    }
}

#[test]
fn the_registry_rejects_a_profile_it_does_not_hold() {
    assert!(
        resolve(Some(&spec("/bin/true", "fixture-engine")), &REGISTRY).is_ok(),
        "зарегистрированный движок разрешается"
    );
    let refusal = resolve(Some(&spec("/bin/true", "fixture")), &REGISTRY)
        .expect_err("движок, которого нет, обязан быть отвергнут");
    assert!(matches!(refusal, ExecutionRefusal::UnknownProfile { .. }));
}

// ── Отказ ДО запуска: ни одного процесса не стартует ──────────────────────────────────

#[test]
fn refusal_happens_before_any_process_is_created() {
    let marker = unique("marker");
    let script = unique("script").with_extension("sh");
    fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).expect("фикстура");
    let program = script.to_string_lossy().into_owned();

    // Все три состояния отказа: ни одно не должно дойти до запуска программы-маркера.
    for bad in [spec(&program, "not-registered"), spec("", "pi")] {
        let _ = resolve(Some(&bad), &REGISTRY).expect_err("отказ до запуска");
    }
    let _ = resolve(None, &REGISTRY).expect_err("отказ до запуска");

    assert!(
        !marker.exists(),
        "программа-маркер не должна была запуститься: {}",
        marker.display()
    );

    let _ = fs::remove_file(&script);
    let _ = fs::remove_file(&marker);
}

// ── Отсутствие поля — не пустое значение ──────────────────────────────────────────────

#[test]
fn a_missing_field_is_a_parse_error_not_an_empty_value() {
    let missing = serde_json::from_str::<ExecutionSpec>(r#"{"program":"/bin/true"}"#);
    assert!(
        missing.is_err(),
        "пропущенный engine — отсутствие данных, а не пустая строка"
    );

    let extra = serde_json::from_str::<ExecutionSpec>(
        r#"{"program":"/bin/true","engine":"pi","surprise":1}"#,
    );
    assert!(extra.is_err(), "лишнее поле отвергается");

    let blank = serde_json::from_str::<ExecutionSpec>(r#"{"program":"","engine":"pi"}"#);
    assert!(
        blank.is_ok(),
        "пустая программа разбирается — её останавливает resolve"
    );
}
