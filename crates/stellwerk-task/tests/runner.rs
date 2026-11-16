//! RED-спецификация раннера: JSON-поток Pi становится доменными событиями.
//!
//! Вердикт различает ПРИЧИНУ отказа: `InvalidLoop`, `InvalidNoToolSteps`,
//! `InvalidTooFewTurns` — разные состояния, а не один флаг (закон 6). Пороги —
//! умолчания `scripts/worker/dispatch.conf:73–75`.
//!
//! Исполнитель ПОДДЕЛЬНЫЙ (packet-09, п. 3): Pi не зовётся, строки задаёт тест, а
//! строка не-JSON обязана стать `MalformedLine`. Импорт — публичный API, не `#[path]`:
//! тот собрал бы вторую копию модуля и оставил реэкспорт непроверенным (F-14).

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{
    DataClass, Event, ModelBinding, PolicyLevel, RetentionClass, RiskClass,
};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::evidence::{tool_attestation, EvidenceEntry, EvidenceLedger, LedgerError};
use stellwerk_task::runner::{
    run_once, run_once_resuming, Call, CallAttempt, CallVerdict, ExecOutcome, ResumeOutcome,
    RiskRegistry, Worker, WorkerError,
};
use stellwerk_task::runner::{stop_reason_from, StopFacts};
use stellwerk_task::runner::{verdict_for, Progress, StopReason, Verdict};
use stellwerk_task::runner::{EventSource, RunnerError};

/// Строки настоящего потока Pi: формат из `scripts/worker/tests/run.sh:41-60`.
const SESSION: &str =
    r#"{"type":"session","version":3,"id":"fix","timestamp":"2026-09-27T00:00:00Z"}"#;
const TURN_END: &str = r#"{"type":"turn_end","toolResults":[],"message":{"stopReason":"stop","usage":{"input":1000,"cacheRead":9000,"cacheWrite":0}}}"#;
const TOOL_START: &str = r#"{"type":"tool_execution_start","toolCallId":"t1","toolName":"bash"}"#;
const NOT_JSON: &str = "{не json";

#[test]
fn clean_run_with_one_step_and_two_turns_is_valid() {
    assert_eq!(verdict_for(StopReason::Stop, 1, 2), Verdict::Valid);
}

#[test]
fn error_stop_is_invalid_because_stop_is_not_ok() {
    let v = verdict_for(StopReason::Error, 1, 2);
    assert_eq!(v, Verdict::InvalidLoop);
    assert_ne!(v, Verdict::InvalidNoToolSteps);
}

#[test]
fn zero_tool_steps_is_its_own_reason() {
    let bad = verdict_for(StopReason::Stop, 0, 2);
    assert_eq!(bad, Verdict::InvalidNoToolSteps, "ни одного действия");
}

#[test]
fn one_turn_is_its_own_reason() {
    let v = verdict_for(StopReason::Stop, 1, 1);
    assert_eq!(v, Verdict::InvalidTooFewTurns);
}

// --- F-18a: причина остановки выводится из фактов процесса, а не выдумывается ---

#[test]
fn our_timeout_with_zero_code_and_no_name_is_timeout() {
    let f = StopFacts {
        timed_out: true,
        exit_code: 0,
        named: None,
    };
    assert_eq!(stop_reason_from(f), StopReason::Timeout);
}

#[test]
fn nonzero_code_outranks_named_stop() {
    let f = StopFacts {
        timed_out: false,
        exit_code: 1,
        named: Some(StopReason::Stop),
    };
    let reason = stop_reason_from(f);
    assert_eq!(
        reason,
        StopReason::Error,
        "провайдер сказал stop, процесс упал"
    );
    assert_ne!(
        reason,
        StopReason::Stop,
        "слово потока не перебивает код возврата"
    );
}

#[test]
fn named_reason_at_exit_zero_survives_verbatim() {
    for named in [StopReason::Stop, StopReason::Error, StopReason::Aborted] {
        let f = StopFacts {
            timed_out: false,
            exit_code: 0,
            named: Some(named),
        };
        assert_eq!(
            stop_reason_from(f),
            named,
            "провайдер назвал — храним ровно это"
        );
    }
}

#[test]
fn our_timeout_outranks_nonzero_code() {
    let f = StopFacts {
        timed_out: true,
        exit_code: 3,
        named: Some(StopReason::Stop),
    };
    let reason = stop_reason_from(f);
    assert_eq!(
        reason,
        StopReason::Timeout,
        "прогон, который мы убили, не бывает успешным"
    );
    assert_ne!(
        reason,
        StopReason::Error,
        "код возврата не отменяет наш таймаут"
    );
}

#[test]
fn silent_exit_zero_is_unreported_and_never_stop() {
    let f = StopFacts {
        timed_out: false,
        exit_code: 0,
        named: None,
    };
    let reason = stop_reason_from(f);
    assert_eq!(reason, StopReason::Unreported, "поток молчал при коде 0");
    assert_ne!(
        reason,
        StopReason::Stop,
        "код 0 — не доказательство, что модель дошла"
    );
}

#[test]
fn unreported_is_distinguishable_by_variant_not_only_by_verdict() {
    for other in [
        StopReason::Stop,
        StopReason::Error,
        StopReason::Timeout,
        StopReason::Aborted,
    ] {
        assert_ne!(
            StopReason::Unreported,
            other,
            "причина видна в evidence по варианту"
        );
    }
    assert_eq!(
        verdict_for(StopReason::Unreported, 1, 2),
        verdict_for(StopReason::Error, 1, 2),
        "вердикт совпадает — различает именно вариант"
    );
}

#[test]
fn unreported_run_is_never_valid() {
    let v = verdict_for(StopReason::Unreported, 1, 2);
    assert_eq!(
        v,
        Verdict::InvalidLoop,
        "молчание — аномалия, гейт обязан её валить"
    );
    assert_ne!(v, Verdict::Valid, "Unreported прошёл как успех");
}

// --- F-18b: счётчики приведены к validity_gate.py, наборы доноров — измеренные ---

#[test]
fn only_tool_execution_start_adds_a_step_and_turn_end_adds_none() {
    let mut p = Progress::default();
    p.absorb(SESSION).expect("сессия — не ошибка");
    assert_eq!(p.tool_steps, 0, "сессия шагом не считается");
    p.absorb(TURN_END).expect("конец хода разбирается");
    assert_eq!(
        p.tool_steps, 0,
        "конец хода шага НЕ даёт — так же и в гейте"
    );
    assert_eq!(p.turns, 0, "ход — это message_end, а не конец хода");
    p.absorb(TOOL_START)
        .expect("начало инструмента разбирается");
    assert_eq!(
        p.tool_steps, 1,
        "единственный донор шага — tool_execution_start"
    );
    p.absorb(TOOL_START)
        .expect("начало инструмента разбирается");
    assert_eq!(p.tool_steps, 2, "второй даёт ещё шаг, а не снова один");
    assert_eq!(p.turns, 0, "ни одно из них не ход");
}

/// Вырезка настоящей трассы `canary-packet-03-20260927-090729`
/// (`scripts/worker/tests/traces/canary-packet-03-20260927-090729.jsonl.gz`, снята
/// 2026-09-27 09:07:29) — события трёх донорских типов в исходном порядке, без
/// повторов. Опущено только то, что раннер не читает: `content`, `sections`,
/// `toolResults`, у `usage` — `cost`/`reasoning`, у `message_end` — всё, кроме `role`.
/// Числа `usage` и `stopReason` — дословно из оригинала, их и сверяют с архивом.
///
/// Сама `.gz` в тест не распаковывается: такой зависимости в `[workspace.dependencies]`
/// нет и добавлять её запрещено (закон 5). Порядок строк и их наборы воспроизводят
/// расхождение сторон: на двух строках `turn_end`, `message_end` и
/// `tool_execution_start` совпадают по счёту и дефект не виден.
const CANARY_EXCERPT: &str = r#"
{"type":"message_end","message":{"role":"system"}}
{"type":"message_end","message":{"role":"user"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":4497,"output":134,"cacheRead":0,"cacheWrite":0,"totalTokens":4631},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a85b66d5f65c6891","toolName":"bash"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a2568af76dbd9335","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":4497,"output":134,"cacheRead":0,"cacheWrite":0,"totalTokens":4631},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":977,"output":218,"cacheRead":4192,"cacheWrite":0,"totalTokens":5387},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-ab0ff1a37bdcddf4","toolName":"bash"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-bc3c17913b1843eb","toolName":"bash"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-874b3632b06d1b75","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":977,"output":218,"cacheRead":4192,"cacheWrite":0,"totalTokens":5387},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1403,"output":253,"cacheRead":4192,"cacheWrite":0,"totalTokens":5848},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a1b2563812c0be23","toolName":"read"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8216b3c1617e7dd3","toolName":"read"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-acea26f03ca19e01","toolName":"read"}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-b484553a9dcc4ab2","toolName":"read"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1403,"output":253,"cacheRead":4192,"cacheWrite":0,"totalTokens":5848},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":3016,"output":94,"cacheRead":4192,"cacheWrite":0,"totalTokens":7302},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-bf5c180a556f430f","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":3016,"output":94,"cacheRead":4192,"cacheWrite":0,"totalTokens":7302},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1446,"output":1220,"cacheRead":6288,"cacheWrite":0,"totalTokens":8954},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9bd80bf44ef9bcff","toolName":"write"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1446,"output":1220,"cacheRead":6288,"cacheWrite":0,"totalTokens":8954},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":631,"output":115,"cacheRead":8384,"cacheWrite":0,"totalTokens":9130},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8b66a046ca8bde0d","toolName":"write"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":631,"output":115,"cacheRead":8384,"cacheWrite":0,"totalTokens":9130},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":807,"output":80,"cacheRead":8384,"cacheWrite":0,"totalTokens":9271},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a7503cf81bff34b8","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":807,"output":80,"cacheRead":8384,"cacheWrite":0,"totalTokens":9271},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1231,"output":200,"cacheRead":8384,"cacheWrite":0,"totalTokens":9815},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-aced9070c499c8f1","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1231,"output":200,"cacheRead":8384,"cacheWrite":0,"totalTokens":9815},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1498,"output":70,"cacheRead":8384,"cacheWrite":0,"totalTokens":9952},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9bfd8f06b1a01a40","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1498,"output":70,"cacheRead":8384,"cacheWrite":0,"totalTokens":9952},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1659,"output":84,"cacheRead":8384,"cacheWrite":0,"totalTokens":10127},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-ab9305f8d21f8b4e","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1659,"output":84,"cacheRead":8384,"cacheWrite":0,"totalTokens":10127},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":4391,"output":631,"cacheRead":8384,"cacheWrite":0,"totalTokens":13406},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9f05dc0110500cd6","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":4391,"output":631,"cacheRead":8384,"cacheWrite":0,"totalTokens":13406},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":897,"output":688,"cacheRead":12576,"cacheWrite":0,"totalTokens":14161},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a73d76a2dd05056b","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":897,"output":688,"cacheRead":12576,"cacheWrite":0,"totalTokens":14161},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1652,"output":197,"cacheRead":12576,"cacheWrite":0,"totalTokens":14425},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8e907629cc5d808d","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1652,"output":197,"cacheRead":12576,"cacheWrite":0,"totalTokens":14425},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1916,"output":76,"cacheRead":12576,"cacheWrite":0,"totalTokens":14568},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9c73e08cd01a51f2","toolName":"read"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1916,"output":76,"cacheRead":12576,"cacheWrite":0,"totalTokens":14568},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":3185,"output":1653,"cacheRead":12576,"cacheWrite":0,"totalTokens":17414},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-bbf42134d7525d70","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":3185,"output":1653,"cacheRead":12576,"cacheWrite":0,"totalTokens":17414},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":713,"output":76,"cacheRead":16768,"cacheWrite":0,"totalTokens":17557},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-bb0f432153ec7f9b","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":713,"output":76,"cacheRead":16768,"cacheWrite":0,"totalTokens":17557},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":880,"output":73,"cacheRead":16768,"cacheWrite":0,"totalTokens":17721},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-b59e5c89652b1ba2","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":880,"output":73,"cacheRead":16768,"cacheWrite":0,"totalTokens":17721},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1689,"output":572,"cacheRead":16768,"cacheWrite":0,"totalTokens":19029},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9604104162ded4d9","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1689,"output":572,"cacheRead":16768,"cacheWrite":0,"totalTokens":19029},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":232,"output":94,"cacheRead":18864,"cacheWrite":0,"totalTokens":19190},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-b15cd9730de1465c","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":232,"output":94,"cacheRead":18864,"cacheWrite":0,"totalTokens":19190},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1218,"output":1338,"cacheRead":18864,"cacheWrite":0,"totalTokens":21420},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-be92a397d003625f","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1218,"output":1338,"cacheRead":18864,"cacheWrite":0,"totalTokens":21420},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":527,"output":73,"cacheRead":20960,"cacheWrite":0,"totalTokens":21560},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-877ddc60c891c8dc","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":527,"output":73,"cacheRead":20960,"cacheWrite":0,"totalTokens":21560},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1287,"output":1183,"cacheRead":20960,"cacheWrite":0,"totalTokens":23430},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a4bcee0a7616f08c","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1287,"output":1183,"cacheRead":20960,"cacheWrite":0,"totalTokens":23430},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":441,"output":73,"cacheRead":23056,"cacheWrite":0,"totalTokens":23570},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8ec7cdc7291a3825","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":441,"output":73,"cacheRead":23056,"cacheWrite":0,"totalTokens":23570},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":2410,"output":1306,"cacheRead":23056,"cacheWrite":0,"totalTokens":26772},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8a1dcba790fff0d6","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":2410,"output":1306,"cacheRead":23056,"cacheWrite":0,"totalTokens":26772},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":1687,"output":73,"cacheRead":25152,"cacheWrite":0,"totalTokens":26912},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-bf029ee2716d7237","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":1687,"output":73,"cacheRead":25152,"cacheWrite":0,"totalTokens":26912},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":2647,"output":1455,"cacheRead":25152,"cacheWrite":0,"totalTokens":29254},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-9accd11a18b56b98","toolName":"edit"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":2647,"output":1455,"cacheRead":25152,"cacheWrite":0,"totalTokens":29254},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":2073,"output":73,"cacheRead":27248,"cacheWrite":0,"totalTokens":29394},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-b8c8da94c9cebb64","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":2073,"output":73,"cacheRead":27248,"cacheWrite":0,"totalTokens":29394},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":495,"output":133,"cacheRead":29344,"cacheWrite":0,"totalTokens":29972},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-88c19e516b23e97a","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":495,"output":133,"cacheRead":29344,"cacheWrite":0,"totalTokens":29972},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":29325,"output":89,"cacheRead":0,"cacheWrite":0,"totalTokens":29414},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-96fb2df715723977","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":29325,"output":89,"cacheRead":0,"cacheWrite":0,"totalTokens":29414},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":161,"output":74,"cacheRead":29344,"cacheWrite":0,"totalTokens":29579},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-b00ab21237b87725","toolName":"read"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":161,"output":74,"cacheRead":29344,"cacheWrite":0,"totalTokens":29579},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":29589,"output":63,"cacheRead":0,"cacheWrite":0,"totalTokens":29652},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-be0c817bfdedc240","toolName":"read"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":29589,"output":63,"cacheRead":0,"cacheWrite":0,"totalTokens":29652},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":371,"output":86,"cacheRead":29344,"cacheWrite":0,"totalTokens":29801},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-93d153d96f37f8bb","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":371,"output":86,"cacheRead":29344,"cacheWrite":0,"totalTokens":29801},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":490,"output":69,"cacheRead":29344,"cacheWrite":0,"totalTokens":29903},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-a3a850d49eca0988","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":490,"output":69,"cacheRead":29344,"cacheWrite":0,"totalTokens":29903},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":627,"output":127,"cacheRead":29344,"cacheWrite":0,"totalTokens":30098},"stopReason":"toolUse"}}
{"type":"tool_execution_start","toolCallId":"chatcmpl-tool-8c1bc459c561bec9","toolName":"bash"}
{"type":"message_end","message":{"role":"toolResult"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":627,"output":127,"cacheRead":29344,"cacheWrite":0,"totalTokens":30098},"stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","usage":{"input":29746,"output":153,"cacheRead":0,"cacheWrite":0,"totalTokens":29899},"stopReason":"stop"}}
{"type":"turn_end","message":{"role":"assistant","usage":{"input":29746,"output":153,"cacheRead":0,"cacheWrite":0,"totalTokens":29899},"stopReason":"stop"}}
"#;

#[test]
fn real_canary_trace_replay_gives_the_same_numbers_as_validity_gate() {
    let mut p = Progress::default();
    let mut f = Fake::of_lines(CANARY_EXCERPT);
    p.drain(&mut f).expect("вырезка разбирается целиком");
    // Числа — вывод `validity_gate.py --jsonl canary-packet-03-20260927-090729.jsonl.gz`.
    assert_eq!(p.tool_steps, 40, "tool_execution_start, а не конец хода");
    assert_eq!(p.turns, 77, "message_end: ходов больше, чем концов хода");
    assert_eq!(
        p.total_input_tokens, 655_622,
        "message.usage всех turn_end: input 135 814 + cacheRead 519 808"
    );
    assert!(
        p.total_input_tokens > 29_971,
        "usage инкрементальный (максимум на событие 29 971) — раннер складывает, \
         а не берёт последний: расхождение с гейтом было в наборе доноров, не в арифметике"
    );

    // Причина выбрана так, чтобы расхождение счётчиков было видно без её участия:
    // последний turn_end объявляет `stop`, а расхождение было и на 34 предыдущих.
    assert_eq!(
        p.stop_reason,
        Some(StopReason::Stop),
        "хвост трассы объявил успех"
    );
}

#[test]
fn clean_stream_through_fake_executor_counts_its_steps() {
    let mut p = Progress::default();
    let mut f = Fake::of(&[SESSION, TOOL_START, TOOL_START, TURN_END]);
    p.drain(&mut f).expect("чистый поток читается");
    assert_eq!(p.tool_steps, 2, "по шагу на каждый tool_execution_start");
    assert_eq!(p.stop_reason, Some(StopReason::Stop), "причина названа");
}

#[test]
fn malformed_line_is_its_own_state_and_does_not_vanish() {
    let mut p = Progress::default();
    let mut f = Fake::of(&[TOOL_START, NOT_JSON, TOOL_START]);
    let refused = p.drain(&mut f).expect_err("строка видна");
    match refused {
        RunnerError::MalformedLine { line_no, line } => {
            assert_eq!(line_no, 2, "битая — вторая, нумерация с единицы");
            assert_eq!(line, NOT_JSON, "текст сохранён: терять его нельзя");
        }
        other => panic!("не-JSON — не отказ песочницы: {other:?}"),
    }
    assert_eq!(p.tool_steps, 1, "шаг до битой засчитан; пропуск дал бы 2");
}

struct Fake(std::vec::IntoIter<String>);

impl Fake {
    fn of(lines: &[&str]) -> Self {
        let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        Self(owned.into_iter())
    }

    /// Поток из одного многострочного блока — так в тест встраивается вырезка трассы.
    fn of_lines(block: &str) -> Self {
        Self(
            block
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<String>>()
                .into_iter(),
        )
    }
}

impl EventSource for Fake {
    fn next_line(&mut self) -> Option<String> {
        self.0.next()
    }
}

// --- O02: `Suspended` и `Deny` — разные состояния с разной причиной в evidence ---

/// Запусков не должно быть ни в одном случае: счётчик отделён от факта записи, иначе
/// «воркер не вызван» прошёл бы и при молчащем раннере.
struct FakeWorker {
    launches: Cell<usize>,
    registry: TestRegistry,
}

impl FakeWorker {
    fn new() -> Self {
        Self {
            launches: Cell::new(0),
            registry: TestRegistry,
        }
    }
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

impl Worker for FakeWorker {
    fn exec(&self, _workspace_ref: &str, _call: &Call) -> Exec<'_> {
        self.launches.set(self.launches.get() + 1);
        Box::pin(ready(Ok(ExecOutcome {
            output: String::new(),
        })))
    }

    fn spawned(&self) -> usize {
        self.launches.get()
    }

    fn registry(&self) -> &dyn RiskRegistry {
        &self.registry
    }
}

/// Доверенный реестр: эффективный риск приходит отсюда, а не из самодекларации вызова.
struct TestRegistry;

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        match (tool, module) {
            ("r2", "fs") => Ok(ToolRisk::Classified(RiskClass::R2)),
            _ => Err(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            }),
        }
    }
}

/// Binding, пропускающий `Public`: иначе отказ по данным замаскировал бы различие
/// между «некому спросить» и «запрещено политикой».
fn holds_any_data() -> ModelBinding {
    ModelBinding {
        id: "b".into(),
        provider: "neuraldeep".into(),
        model_id: "m".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn attempt(task: &str, interactive: bool, caller: PolicyLevel) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: "r2".into(),
            module: "fs".into(),
            risk: ToolRisk::Classified(RiskClass::R2),
            data: DataClass::Public,
            model: holds_any_data(),
            interactive,
        },
        caller,
        workspace_ref: "ws-stellwerk".into(),
    }
}

/// Один и тот же неинтерактивный вызов R2: у `L3Maintainer` спросить некого, поэтому
/// `Suspended`; у `L2Contributor` уровня не хватает, поэтому `Deny` с другой причиной.
/// Тест ловит именно схлопывание двух фактов, а не отдельный отказ.
#[tokio::test]
async fn suspended_and_deny_are_distinct() {
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = FakeWorker::new();
    let task = "t-o02";
    let waiting = attempt(task, false, PolicyLevel::L3Maintainer);
    let refused = attempt(task, false, PolicyLevel::L2Contributor);

    let paused = run_once(&waiting, &worker, &store).await.expect("пауза");
    let denied = run_once(&refused, &worker, &store).await.expect("отказ");
    assert_eq!(paused, CallVerdict::Suspended, "некому ответить");
    assert!(matches!(denied, CallVerdict::Deny { .. }), "{denied:?}");
    assert_eq!(
        worker.spawned(),
        0,
        "ни пауза, ни отказ не запускают воркер"
    );

    let seen = store.list_by_task(task).await.expect("выборка");
    let event_for = |caller: &str| -> &Event {
        seen.iter()
            .find(|event| {
                event.event_type == "run.call"
                    && event.payload.get("caller").map(String::as_str) == Some(caller)
            })
            .unwrap_or_else(|| panic!("нет чека для {caller}: {seen:?}"))
    };
    let paused_event = event_for("L3Maintainer");
    let denied_event = event_for("L2Contributor");

    // Статусы — ровно те имена, которые пишет production (`CallStatus::name`).
    assert_eq!(
        paused_event.payload.get("status").map(String::as_str),
        Some("Suspended"),
        "пауза записана не своим статусом"
    );
    assert_eq!(
        denied_event.payload.get("status").map(String::as_str),
        Some("Denied"),
        "отказ записан не своим статусом"
    );
    assert_ne!(
        paused_event.payload.get("status"),
        denied_event.payload.get("status"),
        "статусы схлопнулись"
    );

    let paused_reason = paused_event
        .payload
        .get("reason")
        .expect("пауза без причины");
    let denied_reason = denied_event
        .payload
        .get("reason")
        .expect("отказ без причины");
    assert!(!paused_reason.trim().is_empty(), "причина паузы пуста");
    assert!(!denied_reason.trim().is_empty(), "причина отказа пуста");
    assert_ne!(paused_reason, denied_reason, "причины схлопнулись");
}

// --- V2-03: возобновление решается статусом доказательств ДО запуска воркера ---

const RESUME_TASK: &str = "t-resume";
const RESUME_TS: &str = "2026-11-10T00:00:00Z";

/// Реестр, разрешающий R0: нужен, чтобы `Allow` дошёл до РЕАЛЬНОГО вызова воркера,
/// и счётчик запусков что-то доказывал.
#[derive(Default)]
struct AllowRegistry;

impl RiskRegistry for AllowRegistry {
    fn risk_of(&self, _tool: &str, _module: &str) -> Result<ToolRisk, PermissionError> {
        Ok(ToolRisk::Classified(RiskClass::R0))
    }
}

/// Счётный воркер: число запусков отделено от факта записи события, поэтому
/// «воркер не вызван» нельзя вывести из отсутствия чека.
#[derive(Default)]
struct AllowWorker {
    launches: Cell<usize>,
    registry: AllowRegistry,
}

impl Worker for AllowWorker {
    fn exec(&self, _workspace_ref: &str, _call: &Call) -> Exec<'_> {
        self.launches.set(self.launches.get() + 1);
        Box::pin(ready(Ok(ExecOutcome {
            output: String::new(),
        })))
    }

    fn spawned(&self) -> usize {
        self.launches.get()
    }

    fn registry(&self) -> &dyn RiskRegistry {
        &self.registry
    }
}

fn allow_attempt(task: &str) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: "r0".into(),
            module: "fs".into(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: DataClass::Public,
            model: holds_any_data(),
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "ws-stellwerk".into(),
    }
}

/// Леджер из готовой цепочки: тест задаёт статус доказательств, не хранилище.
struct VecLedger(Vec<EvidenceEntry>);

impl EvidenceLedger for VecLedger {
    fn append(&mut self, _entry: &EvidenceEntry) -> Result<(), LedgerError> {
        Ok(())
    }

    fn entries(&self, _task_id: &str) -> Vec<EvidenceEntry> {
        self.0.clone()
    }
}

#[tokio::test]
async fn resume_unattested_proceeds_and_runs_worker_exactly_once() {
    // Given: пустой ledger — предъявлять нечего.
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = AllowWorker::default();
    let ledger = VecLedger(Vec::new());

    // When: возобновление неаттестованного прогона.
    let outcome = run_once_resuming(&allow_attempt(RESUME_TASK), &worker, &store, &ledger)
        .await
        .expect("возобновление");

    // Then: прогон продолжается и воркер запущен ровно один раз.
    assert_eq!(outcome, ResumeOutcome::Proceeded(CallVerdict::Allow));
    assert_eq!(
        worker.spawned(),
        1,
        "неаттестованный прогон — ровно один запуск"
    );
}

#[tokio::test]
async fn resume_attested_without_validation_does_not_run_worker() {
    // Given: удостоверение есть, решения человека нет.
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = AllowWorker::default();
    let ledger = VecLedger(vec![tool_attestation(
        RESUME_TASK,
        "e-tool",
        "cargo test",
        "h-tool",
        RESUME_TS,
    )]);

    // When: возобновление.
    let outcome = run_once_resuming(&allow_attempt(RESUME_TASK), &worker, &store, &ledger)
        .await
        .expect("возобновление");

    // Then: результат ждёт человека, воркер НЕ запускается.
    assert_eq!(outcome, ResumeOutcome::AwaitingHumanValidation);
    assert_eq!(
        worker.spawned(),
        0,
        "ожидание человека не перезапускает прогон"
    );
}
