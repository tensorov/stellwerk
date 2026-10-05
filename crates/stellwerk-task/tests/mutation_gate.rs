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
use std::path::Path;

/// Порог из поля `mutation_threshold` пакета: это приёмка, а не настройка.
const REQUIRED_THRESHOLD: u32 = 80;
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
