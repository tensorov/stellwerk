//! Исполнитель: таймаут, саморекурсия, граница между discovery и исполнением.
//!
//! Проверяется прямой вызов функций модуля на Python, а не запуск CLI: политика cargo
//! живёт в `packet_replay_validate` и уже покрыта packet-09, а исполнитель — общий
//! примитив «запустить argv с таймаутом». Гонять cargo внутри cargo ради проверки
//! примитива было бы дорого и ничего бы не добавляло.
//!
//! Ни одна acceptance-команда здесь не исполняется: все фикстуры — дешёвые процессы,
//! запускаемые во временном каталоге.

use std::path::PathBuf;
use std::process::Command;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join("packet_replay_run.py")
}

/// Выполнить код на Python с путём к модулю исполнителя в sys.path.
///
/// Импорт делает сам тест, а не инструмент: флаг «выполнить произвольный код» в
/// инструменте, который существует ради контролируемого исполнения, был бы тем самым
/// люком, которого в нём быть не должно.
fn run_py(body: &str) -> String {
    let path = script();
    let dir = path.parent().expect("у скрипта есть каталог").to_path_buf();
    let prelude = format!(
        "import sys; sys.path.insert(0, {:?})\nfrom packet_replay_run import *\n",
        dir.to_str().expect("путь в utf-8")
    );
    let out = Command::new("python3")
        .arg("-c")
        .arg(format!("{}{}", prelude, body))
        .output()
        .expect("python3 должен запускаться");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "код на Python упал:\nstdout={}\nstderr={}",
        stdout,
        stderr
    );
    stdout
}

/// Таймаут после частичного успешного вывода обязан давать FAIL и сохранять то,
/// что успело напечататься: cargo к моменту таймаута часто напечатал результат
/// одного бинарника и ещё работал.
///
/// Фикстура печатает с `flush=True` не по привычке, а потому что без него данные теряет
/// сам дочерний процесс: при не-tty stdout `print` пишет в блок-буфер, и убитый процесс
/// этот буфер не сбрасывает. Настоящий cargo и libtest свой вывод сбрасывают, поэтому
/// фикстура обязана это воспроизводить, иначе проверялась бы буферизация Python, а не
/// обработка таймаута.
#[test]
fn timeout_after_partial_output_is_fail_and_keeps_data() {
    let stdout = run_py(
        r#"
import tempfile, json
d = tempfile.mkdtemp()
code = "print('test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out', flush=True); import time; time.sleep(30)"
rec = execute(['python3', '-c', code], d, timeout=2)
print(json.dumps({
    'timed_out': rec['timed_out'],
    'exit_code': rec['exit_code'],
    'has_partial': 'test result: ok. 3 passed' in rec['stdout'],
    'duration_positive': rec['duration_s'] > 0,
}))
"#,
    );
    let got = stdout.trim();
    assert!(
        got.contains("\"timed_out\": true"),
        "таймаут не зафиксирован: {}",
        got
    );
    assert!(
        got.contains("\"has_partial\": true"),
        "частичный вывод при таймауте потерян: {}",
        got
    );
    assert!(
        got.contains("\"duration_positive\": true"),
        "длительность не измерена: {}",
        got
    );
}

/// Таймаут обязан убивать всю группу процесса, а не только прямого ребёнка: cargo
/// порождает rustc и бинарники тестов, и убийство только cargo оставит их работать
/// и писать в тот же каталог.
#[test]
fn timeout_kills_the_whole_process_group() {
    let stdout = run_py(
        r#"
import tempfile, json, os, time
d = tempfile.mkdtemp()
marker = os.path.join(d, 'child_alive')
code = "import os,subprocess,sys,time; subprocess.Popen([sys.executable,'-c','import time; time.sleep(6)']); time.sleep(30)"
rec = execute(['python3', '-c', code], d, timeout=2)
time.sleep(1.0)
print(json.dumps({'timed_out': rec['timed_out']}))
"#,
    );
    assert!(
        stdout.trim().contains("\"timed_out\": true"),
        "таймаут не зафиксирован: {}",
        stdout.trim()
    );
}

/// Команда, содержащая сам replay, исполнена быть не может: это рекурсия, способная
/// выйти за пределы отведённого времени. Молчаливый пропуск недопустим — причина
/// обязана быть названа.
#[test]
fn replay_recursion_is_refused_with_reason() {
    let stdout = run_py(
        r#"
import json
verdict = refuse_recursion(['python3', 'scripts/packet_replay.py', 'inventory'])
print(json.dumps({'verdict': verdict}))
"#,
    );
    let got = stdout.trim();
    assert!(
        got.contains("RECURSION"),
        "саморекурсия обязана быть опознана и названа: {}",
        got
    );
}

/// Обычная команда рекурсией не считается: отказ должен быть точным, а не «на всякий случай».
#[test]
fn ordinary_command_is_not_refused_as_recursion() {
    let stdout = run_py(
        r#"
import json
print(json.dumps({'verdict': refuse_recursion(['cargo', 'test', '-p', 'stellwerk-registry'])}))
"#,
    );
    assert_eq!(stdout.trim(), "{\"verdict\": \"\"}", "{}", stdout.trim());
}

/// Discovery обязан сохранить условия отбора, иначе он измеряет не ту выборку,
/// о которой отчёт утверждает.
#[test]
fn discovery_argv_preserves_selection_conditions() {
    let stdout = run_py(
        r#"
import json
base = ['cargo', 'test', '-p', 'stellwerk-registry', 'measure']
print(json.dumps({
    'plain': discovery_argv(base),
    'with_target': discovery_argv(['cargo', 'test', '-p', 'x', '--test', 'y', 'f']),
}))
"#,
    );
    let got = stdout.trim();
    assert!(
        got.contains(
            r#""plain": ["cargo", "test", "-p", "stellwerk-registry", "measure", "--", "--list"]"#
        ),
        "условия отбора не сохранены или --list добавлен не в конец: {}",
        got
    );
    assert!(
        got.contains(r#""--test", "y", "f", "--", "--list"]"#),
        "цель или фильтр потеряны при discovery: {}",
        got
    );
}

/// `selected_by_list` и результат исполнения — разные величины. Совпадение нулей не
/// должно выглядеть как «проверено всё».
#[test]
fn selected_by_list_never_becomes_executed_count() {
    let stdout = run_py(
        r#"
import json
from packet_replay_aggregate import classify, parse_result_lines
totals = parse_result_lines("test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 71 filtered out\n")
print(json.dumps({
    'with_selection': classify(totals, 0, False, 71),
    'without_selection': classify(totals, 0, False, None),
}))
"#,
    );
    let got = stdout.trim();
    assert!(
        got.contains("\"with_selection\": \"INCONCLUSIVE_ZERO_EXECUTED\""),
        "71 найденный тест не должен превращаться в выполненный: {}",
        got
    );
    assert!(
        got.contains("\"without_selection\": \"INCONCLUSIVE_ZERO_EXECUTED\""),
        "пустой прогон обязан остаться неоднозначным: {}",
        got
    );
}
