//! GREEN M5/packet-18a — порог 80% как код, а не строка в приёмке.
//!
//! Контракт счёта — `mutants.out/outcomes.json`, и формат взят из исходников 27.1.0:
//! счётчики `LabOutcome` суть `usize` (outcome.rs:60-72); `end_time` заполняется только
//! в `finish()` (output.rs:247), хотя файл переписывается после каждого сценария
//! (output.rs:199-206); `add()` кладёт каждого мутанта ровно в один bucket, а `Failure`
//! увеличивает `total_mutants` и ни один счётчик (outcome.rs:92-105). Отсюда три проверки
//! целостности: версия формата, `end_time`, сумма счётчиков. Счёт целочисленный:
//! `4/5*100` в f64 это 80.00000000000001. Код возврата cargo-mutants в решении не
//! участвует: `exit_code()` (outcome.rs:110-125) спрашивает «есть ли непойманный
//! мутант», а не «набран ли порог». Замеров cargo-mutants нет — слишком долго, всё
//! гонится на фикстурах.

use serde_json::Value;
use std::fs;
use std::path::Path;

/// Порог из поля `mutation_threshold` пакета: это приёмка, а не настройка.
const REQUIRED_THRESHOLD: u32 = 80;
/// Гейт живёт только на критичных крейтех; `stellwerk-store` диагностический.
const CRITICAL_CRATES: [&str; 2] = ["stellwerk-gateway", "stellwerk-task"];
const TASK: &str = "stellwerk-task";
const ARTIFACT: &str = "mutants.out/outcomes.json";
const VERSION: &str = "\"cargo_mutants_version\":\"27.1.0\",";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counts {
    caught: u64,
    missed: u64,
    timeout: u64,
    /// Вне знаменателя: подмена тела trait-impl на `Default::default()` не компилируется.
    unviable: u64,
    success: u64,
    total: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum GateError {
    NotCritical(String),
    NoArtifact(String),
    Malformed(String),
    Incomplete(String),
}

/// Ноль viable мутантов это N/A, а не 0% и не 100%: измерения нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Pass,
    Fail,
    NotApplicable,
}

#[derive(Debug, PartialEq, Eq)]
struct Gate {
    decision: Decision,
    /// Сотые процента; `None` это N/A.
    hundredths: Option<u32>,
    message: String,
}

/// Вход гейта: каталог прогона и код возврата cargo-mutants, который в решении не
/// участвует и переносится только в диагностику.
struct Run<'a> {
    outcomes: &'a Path,
    tool_exit: i32,
}

/// `caught / (caught + missed + timeout) * 100` в сотых, с округлением: `unviable`
/// вне знаменателя, `timeout` не приравнивается к `caught`.
fn score_hundredths(c: Counts) -> Option<u32> {
    let denom = c.caught + c.missed + c.timeout;
    if denom == 0 {
        return None;
    }
    let scaled = (c.caught * 10_000 + denom / 2) / denom;
    debug_assert!(scaled <= 10_000, "доля не превышает 100%");
    Some(scaled as u32)
}

/// Сравнение целыми: ровно на пороге это PASS, а не FAIL.
fn decide(hundredths: Option<u32>, threshold: u32) -> Decision {
    match hundredths {
        None => Decision::NotApplicable,
        Some(h) if h >= threshold * 100 => Decision::Pass,
        Some(_) => Decision::Fail,
    }
}

/// Понижение порога отвергается, а не применяется; выше 100 недостижим и потому тоже.
fn threshold(percent: u32) -> Result<u32, GateError> {
    match percent {
        REQUIRED_THRESHOLD..=100 => Ok(percent),
        other => Err(GateError::Malformed(format!(
            "порог {other}% отвергнут: mutation_threshold пакета = {REQUIRED_THRESHOLD}%"
        ))),
    }
}

fn parse_outcomes(raw: &str) -> Result<Counts, GateError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| GateError::Malformed(e.to_string()))?;
    // Версия — часть контракта: неизвестный формат это отсутствие измерения, а не ноль.
    let version = v.get("cargo_mutants_version").and_then(Value::as_str);
    if version.is_none() {
        return Err(GateError::Malformed("нет cargo_mutants_version".to_owned()));
    }
    match v.get("end_time") {
        None | Some(Value::Null) => return Err(GateError::Incomplete("end_time пуст".to_owned())),
        Some(_) => {}
    }
    let n = |key: &str| -> Result<u64, GateError> {
        let counter = v.get(key).and_then(Value::as_u64);
        counter.ok_or_else(|| GateError::Malformed(format!("поле {key} не счётчик usize")))
    };
    let c = Counts {
        caught: n("caught")?,
        missed: n("missed")?,
        timeout: n("timeout")?,
        unviable: n("unviable")?,
        success: n("success")?,
        total: n("total_mutants")?,
    };
    // Каждый мутант в ровно одном bucket, иначе часть прогона unattributed и счёт неполон.
    let buckets = c.caught + c.missed + c.timeout + c.unviable + c.success;
    if buckets != c.total {
        return Err(GateError::Incomplete(format!(
            "total_mutants={} не равно сумме счётчиков={buckets}",
            c.total
        )));
    }
    Ok(c)
}

fn gate(run: &Run<'_>, crate_name: &str, percent: u32) -> Result<Gate, GateError> {
    let threshold = threshold(percent)?;
    if !CRITICAL_CRATES.contains(&crate_name) {
        return Err(GateError::NotCritical(crate_name.to_owned()));
    }
    let raw = fs::read_to_string(run.outcomes.join(ARTIFACT))
        .map_err(|e| GateError::NoArtifact(e.to_string()))?;
    let counts = parse_outcomes(&raw)?;
    let hundredths = score_hundredths(counts);
    let decision = decide(hundredths, threshold);
    let reached = hundredths.map_or_else(|| "N/A (измерения нет)".to_owned(), show_percent);
    let label = match decision {
        Decision::Pass => "PASS",
        Decision::Fail => "FAIL",
        Decision::NotApplicable => "N/A",
    };
    let message = format!(
        "{label} {crate_name}: {reached} против порога {}; caught {}, missed {}, timeout {}, \
         unviable {} вне знаменателя; код cargo-mutants {} в решении не участвует",
        show_percent(threshold * 100),
        counts.caught,
        counts.missed,
        counts.timeout,
        counts.unviable,
        run.tool_exit,
    );
    Ok(Gate {
        decision,
        hundredths,
        message,
    })
}

fn show_percent(h: u32) -> String {
    format!("{}.{:02}%", h / 100, h % 100)
}

/// `outcomes.json` по реальному набору полей `LabOutcome`.
fn outcome_json(caught: u64, missed: u64, timeout: u64, unviable: u64) -> String {
    serde_json::json!({
        "caught": caught, "missed": missed, "timeout": timeout, "unviable": unviable,
        "success": 0, "total_mutants": caught + missed + timeout + unviable,
        "start_time": "2026-10-06T00:00:00Z", "end_time": "2026-10-06T00:04:00Z",
        "cargo_mutants_version": "27.1.0", "outcomes": []
    })
    .to_string()
}

/// Каталог прогона с ложной приманкой: stdout рядом обещает «4 caught» и 80.00%, но
/// stdout не контракт, и числа берутся из outcomes.json.
fn gate_for(tag: &str, crate_name: &str, json: Option<&str>, exit: i32) -> Result<Gate, GateError> {
    let dir = std::env::temp_dir().join(format!("sw-mut-{tag}-{}", std::process::id()));
    let artifact = dir.join(ARTIFACT);
    fs::create_dir_all(artifact.parent().expect("у артефакта есть каталог")).expect("каталог есть");
    let decoy = "5 mutants tested: 4 caught\ntest result: ok. 80.00%\n";
    fs::write(dir.join("stdout.txt"), decoy).expect("stdout пишется");
    if let Some(json) = json {
        fs::write(&artifact, json).expect("артефакт пишется");
    }
    let run = Run {
        outcomes: &dir,
        tool_exit: exit,
    };
    gate(&run, crate_name, REQUIRED_THRESHOLD)
}

/// Гейт на критичном крейте по готовому замеру; код возврата 2 (FoundProblems).
fn scored(
    tag: &str,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
) -> Result<Gate, GateError> {
    gate_for(
        tag,
        TASK,
        Some(&outcome_json(caught, missed, timeout, unviable)),
        2,
    )
}

/// clause 1: искусственно заниженная оценка даёт провал с именем крейта, порогом и
/// достигнутым значением; числа — замер пакета на HEAD 6ca1934.
#[test]
fn clause_1_low_score_fails_with_crate_threshold_and_reached() {
    let g = scored("c1", 133, 81, 54, 19).expect("гейт отработал");
    assert_eq!((g.decision, g.hundredths), (Decision::Fail, Some(4963)));
    let m = &g.message;
    assert!(
        ["stellwerk-task", "80.00%", "49.63%"]
            .iter()
            .all(|n| m.contains(n)),
        "{m}"
    );
}

/// clauses 2, 3, 6: score целыми и с округлением; столбец — (caught, missed, timeout,
/// unviable) → (сотые, решение).
#[test]
fn score_is_rounded_integer_timeout_is_not_caught_unviable_excluded() {
    let cases = [
        ((4, 1, 0, 0), Some(8000), Decision::Pass), // clause 2: ровно порог
        ((4, 1, 0, 500), Some(8000), Decision::Pass), // clause 6: unviable вне знаменателя
        ((1, 0, 1, 0), Some(5000), Decision::Fail), // clause 3: timeout не caught
        ((0, 0, 1, 0), Some(0), Decision::Fail),    // clause 3: один timeout — не N/A
        ((79, 1, 0, 0), Some(9875), Decision::Pass), // округление, а не усечение
    ];
    for ((c, m, t, u), want, d) in cases {
        let g = scored("t", c, m, t, u).expect("гейт отработал");
        assert_eq!((g.hundredths, g.decision), (want, d), "{c}/{m}/{t}/{u}");
    }
}

/// clause 5: нулевой знаменатель даёт N/A, а не 100% и не 0% — состояния разные.
#[test]
fn clause_5_zero_denominator_is_na_not_100() {
    let g = scored("c5", 0, 0, 0, 19).expect("гейт отработал");
    assert_eq!((g.decision, g.hundredths), (Decision::NotApplicable, None));
    assert!(
        g.message.contains("N/A") && !g.message.contains("100.00%"),
        "{}",
        g.message
    );
}

/// clause 4: неполный или ошибочный прогон не даёт PASS.
#[test]
fn clause_4_incomplete_or_erroneous_run_never_passes() {
    let good = outcome_json(4, 1, 0, 0);
    // Оборванный прогон: end_time есть только у дошедшего до finish(), и счётчики
    // обязаны сойтись с total_mutants, иначе часть мутантов unattributed.
    for bad in [
        good.replace("\"end_time\":\"2026-10-06T00:04:00Z\"", "\"end_time\":null"),
        good.replace("\"total_mutants\":5", "\"total_mutants\":9"),
    ] {
        assert!(
            matches!(parse_outcomes(&bad), Err(GateError::Incomplete(_))),
            "{bad}"
        );
    }
    // Ошибочный формат: usize не бывает отрицательным, версия обязательна, мусор не парсится.
    for bad in [
        good.replace("\"caught\":4", "\"caught\":-4"),
        good.replace(VERSION, ""),
        "{не json".to_owned(),
    ] {
        assert!(
            matches!(parse_outcomes(&bad), Err(GateError::Malformed(_))),
            "{bad}"
        );
    }
}

/// clause 7: понижение порога ниже 80 отвергается, а не применяется.
#[test]
fn clause_7_lowering_threshold_is_rejected() {
    assert_eq!(REQUIRED_THRESHOLD, 80);
    assert!(threshold(80).is_ok() && threshold(79).is_err() && threshold(0).is_err());
    // Контракт порога проверяется до чтения артефакта: 79 отвергается, даже когда файла нет.
    let run = Run {
        outcomes: &std::env::temp_dir(),
        tool_exit: 2,
    };
    assert!(matches!(gate(&run, TASK, 79), Err(GateError::Malformed(_))));
    assert!(matches!(gate(&run, TASK, 40), Err(GateError::Malformed(_))));
}

/// clause 8: счёт читается из outcomes.json, а не из stdout — stdout не контракт.
#[test]
fn clause_8_score_comes_from_outcomes_json_not_stdout() {
    let g = scored("c8", 2, 1, 0, 0).expect("гейт отработал");
    assert_eq!((g.decision, g.hundredths), (Decision::Fail, Some(6667)));
    assert!(
        g.message.contains("66.67%") && g.message.contains("caught 2"),
        "{}",
        g.message
    );
    assert!(
        !g.message.contains("4 caught"),
        "приманка из stdout не должна считаться"
    );
    assert!(matches!(
        gate_for("c8-bare", TASK, None, 0),
        Err(GateError::NoArtifact(_))
    ));
}

/// clause 9: код возврата cargo-mutants сам по себе не признак прохождения; 4/1 — ровно
/// порог, при котором инструмент вернул бы FoundProblems(2).
#[test]
fn clause_9_tool_exit_code_is_not_the_pass_signal() {
    let verdicts = |tag: &str, c: u64, m: u64, t: u64, u: u64| -> Vec<Decision> {
        [0, 2, 3, 4, 70]
            .iter()
            .map(|e| {
                gate_for(tag, TASK, Some(&outcome_json(c, m, t, u)), *e)
                    .expect("отработал")
                    .decision
            })
            .collect()
    };
    assert_eq!(verdicts("c9", 4, 1, 0, 0), vec![Decision::Pass; 5]);
    assert_eq!(verdicts("c9-low", 133, 81, 54, 19), vec![Decision::Fail; 5]);
}

/// forbidden: проверка не запускается на не критическом крейте даже с идеальным замером.
#[test]
fn gate_refuses_non_critical_crate() {
    let perfect = gate_for(
        "crit",
        "stellwerk-store",
        Some(&outcome_json(10, 0, 0, 0)),
        0,
    );
    assert_eq!(
        perfect,
        Err(GateError::NotCritical("stellwerk-store".to_owned()))
    );
    let ok = |name: &str| gate_for("crit", name, Some(&outcome_json(10, 0, 0, 0)), 0).is_ok();
    assert!(ok("stellwerk-gateway") && ok(TASK));
}
