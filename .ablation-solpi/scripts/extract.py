#!/usr/bin/env python3
"""extract.py — Phase B, B6. Все метрики Фазы A из Pi `--mode json` вывода.

Без внешних зависимостей. Не печатает содержимое prompt/transcript — только числа.

Схема событий (проверена на реальном выводе Pi 0.87.0, --mode json):
  message_end.message.usage {input,output,cacheRead,cacheWrite,reasoning,totalTokens,cost}
  turn_end.message.*       — те же usage + stopReason (дубликат message_end, НЕ суммируется)
  tool_execution_start     {toolCallId, toolName, args}
  compaction_start         {reason}
Источник usage — message_end (Ф3). turn_end используется только как cross-check:
совпадение сумм фиксируется в usage_match, расхождение — в usage_match=false.
"""
import argparse
import json
import os
import re
import sys

# 8 тестов model.rs на BASE 2d572f1 (base_preflight: 8 passed)
BASE_MODEL_TESTS = (
    'from_toml_str_parses_fixture',
    'from_toml_str_rejects_unknown_module_keys',
    'from_toml_str_rejects_unknown_source_keys',
    'load_reads_file',
    'source_roundtrip_crate',
    'source_roundtrip_git',
    'source_roundtrip_npm',
    'tokens_for_formula',
)

VAL_KEYS = ('input', 'cacheRead', 'cacheWrite', 'output')


def ctx_of(usage):
    return int(usage.get('input', 0) or 0) + int(usage.get('cacheRead', 0) or 0) + \
           int(usage.get('cacheWrite', 0) or 0)


def nonempty_then_run(val):
    if isinstance(val, dict) and val:
        return val
    if isinstance(val, str) and val.strip():
        return val
    return None


def find_then_run(obj):
    """Считает then_run ТОЛЬКО в структурах аргументов вызова тула.

    Наивный поиск по всему событию давал ложные срабатывания: строка "then_run"
    встречается в прозе workload-а ("если поддерживает параметр then_run"), и QA
    дал af_fired=6 при af_fired=0 по факту. Поэтому берём только:
      * tool_execution_start -> args.then_run
      * блоки toolCall/tool_call в message.content -> arguments|args .then_run
    """
    hits = []
    if obj.get('type') == 'tool_execution_start':
        val = nonempty_then_run((obj.get('args') or {}).get('then_run'))
        if val is not None:
            hits.append(val)
    for key in ('content', 'message'):
        node = obj.get(key)
        blocks = node if isinstance(node, list) else (
            [node.get('content')] if key == 'message' and isinstance(node, dict)
            and isinstance(node.get('content'), list) else [])
        for b in blocks or []:
            if not isinstance(b, dict):
                continue
            if b.get('type') not in ('toolCall', 'tool_call'):
                continue
            for ak in ('arguments', 'args'):
                sub = b.get(ak)
                if isinstance(sub, str):
                    try:
                        sub = json.loads(sub)
                    except (json.JSONDecodeError, TypeError):
                        continue
                val = nonempty_then_run((sub or {}).get('then_run')) if isinstance(sub, dict) else None
                if val is not None:
                    hits.append(val)
    return hits


def scan_jsonl(path):
    m = {
        'message_end_events': 0, 'turn_end_events': 0,
        'total_input_cum': 0, 'total_output_cum': 0,
        'total_input_cum_turnend': 0, 'total_output_cum_turnend': 0,
        'cache_read_cum': 0, 'cache_write_cum': 0, 'reasoning_cum': 0,
        'peak_context': 0, 'final_context': 0, 'peak_total_tokens': 0,
        'tool_steps': 0, 'tool_names': {}, 'turns': 0, 'compactions': 0,
        'stop_reason_last': None, 'models': [],
        'af_fired': 0, 'obs_recall_calls': 0, 'unparsed_lines': 0,
    }
    if not os.path.exists(path):
        return m, 0
    lines = 0
    with open(path, 'r', errors='replace') as fh:
        for raw in fh:
            raw = raw.strip()
            if not raw:
                continue
            lines += 1
            try:
                obj = json.loads(raw)
            except json.JSONDecodeError:
                m['unparsed_lines'] += 1
                continue
            hits = find_then_run(obj)
            if hits:
                m['af_fired'] += len(hits)
            etype = obj.get('type')
            if etype == 'tool_execution_start':
                m['tool_steps'] += 1
                name = obj.get('toolName') or '<none>'
                m['tool_names'][name] = m['tool_names'].get(name, 0) + 1
                if name == 'obs_recall':
                    m['obs_recall_calls'] += 1
            elif etype == 'compaction_start':
                m['compactions'] += 1
            elif etype in ('message_end', 'turn_end'):
                msg = obj.get('message') or {}
                usage = msg.get('usage') or {}
                model = msg.get('model')
                if model and model not in m['models']:
                    m['models'].append(model)
                if not isinstance(usage, dict) or not usage:
                    continue
                c = ctx_of(usage)
                out = int(usage.get('output', 0) or 0)
                if etype == 'message_end':
                    m['message_end_events'] += 1
                    m['turns'] = m['message_end_events']
                    m['total_input_cum'] += c
                    m['total_output_cum'] += out
                    m['cache_read_cum'] += int(usage.get('cacheRead', 0) or 0)
                    m['cache_write_cum'] += int(usage.get('cacheWrite', 0) or 0)
                    m['reasoning_cum'] += int(usage.get('reasoning', 0) or 0)
                    if c > m['peak_context']:
                        m['peak_context'] = c
                    m['final_context'] = c
                    tt = int(usage.get('totalTokens', 0) or 0)
                    if tt > m['peak_total_tokens']:
                        m['peak_total_tokens'] = tt
                else:
                    m['turn_end_events'] += 1
                    m['total_input_cum_turnend'] += c
                    m['total_output_cum_turnend'] += out
                    if msg.get('stopReason'):
                        m['stop_reason_last'] = msg.get('stopReason')
    return m, lines


def scan_ledger(session_dir):
    """ObservationPack: <session-dir>/sol-pi/<session-id>/observation-pack/ledger.jsonl"""
    out = {
        'op_fired': 0, 'op_placeholder': 0, 'op_recall': 0, 'op_full': 0,
        'op_removed_tokens': 0, 'op_original_tokens': 0,
        'ledger_files': 0, 'ledger_entries': 0,
        'solpi_compactions': 0, 'epr_records': 0,
    }
    if not session_dir or not os.path.isdir(session_dir):
        return out
    base = os.path.join(session_dir, 'sol-pi')
    if not os.path.isdir(base):
        return out
    for dirpath, _dirs, files in os.walk(base):
        if 'ledger.jsonl' not in files:
            continue
        out['ledger_files'] += 1
        with open(os.path.join(dirpath, 'ledger.jsonl'), errors='replace') as fh:
            for raw in fh:
                raw = raw.strip()
                if not raw:
                    continue
                try:
                    rec = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                out['ledger_entries'] += 1
                ev = rec.get('event')
                if ev == 'placeholder':
                    out['op_placeholder'] += 1
                    out['op_removed_tokens'] += int(rec.get('removedTokens', 0) or 0)
                    out['op_original_tokens'] += int(rec.get('originalTokens', 0) or 0)
                elif ev == 'recall':
                    out['op_recall'] += 1
                elif ev == 'full':
                    out['op_full'] += 1
    out['op_fired'] = out['op_placeholder'] + out['op_recall']
    # SoL-Pi-компакции живут в online-context-compact/ (отключена флагом), а
    # Evidence-Preserving Reducer — в evidence-preserving-reducer/ (тоже выключен).
    # Считаем их наличие, чтобы доказать, что они не участвовали.
    for sub, key in (('online-context-compact', 'solpi_compactions'),
                     ('evidence-preserving-reducer', 'epr_records')):
        d = os.path.join(base, sub)
        if not os.path.isdir(d):
            continue
        for dirpath, _dirs, files in os.walk(d):
            for fn in files:
                fp = os.path.join(dirpath, fn)
                if fn.endswith(('.json', '.jsonl')):
                    out[key] += sum(1 for ln in open(fp, errors='replace') if ln.strip())
                else:
                    out[key] += 1
    return out


def read_text(path):
    try:
        with open(path, errors='replace') as fh:
            return fh.read()
    except OSError:
        return ''


def parse_tests(path):
    res = {'tests_exit': None, 'tests_last_line': '', 'w1_names_present': [],
           'w1_names_missing': [], 'tests_summary_line': '',
           'tests_result_registry': '', 'tests_passed': None, 'tests_failed': None,
           'base_tests_present': [], 'base_tests_missing': []}
    txt = read_text(path)
    if not txt.strip():
        return res
    m = read_text(path + '.rc').strip()
    res['tests_exit'] = int(m) if m.isdigit() else None
    lines = txt.splitlines()
    for i, line in enumerate(lines):
        s = line.strip()
        if 'stellwerk_registry-' in s and 'Running' in s:
            res['tests_summary_line'] = s
            for follow in lines[i + 1:]:
                f = follow.strip()
                if f.startswith('test result:'):
                    res['tests_result_registry'] = f
                    m = re.search(r'ok\.\s+(\d+) passed;\s+(\d+) failed', f)
                    if m:
                        res['tests_passed'] = int(m.group(1))
                        res['tests_failed'] = int(m.group(2))
                    break
    res['tests_last_line'] = next(
        (l.strip() for l in reversed(lines) if l.strip().startswith('test result:')), '')
    for name in ('w1_alpha', 'w1_beta', 'w1_gamma'):
        (res['w1_names_present'] if name in txt else res['w1_names_missing']).append(name)
    for name in BASE_MODEL_TESTS:
        (res['base_tests_present'] if name in txt else res['base_tests_missing']).append(name)
    return res


def parse_diff(path):
    res = {'diff_files_changed': 0, 'diff_insertions': 0, 'diff_deletions': 0,
           'files_touched': [], 'diff_untracked': []}
    txt = read_text(path)
    ins = dele = 0
    for line in txt.splitlines():
        if line.startswith('+++ ') or line.startswith('--- '):
            continue
        parts = line.split('\t')
        if len(parts) >= 3:
            try:
                a, b = int(parts[0]), int(parts[1])
            except ValueError:
                continue
            ins += a
            dele += b
            res['files_touched'].append(parts[2])
        elif line.startswith('?? '):
            res['diff_untracked'].append(line[3:].strip())
    res['diff_files_changed'] = len(res['files_touched'])
    res['diff_insertions'] = ins
    res['diff_deletions'] = dele
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--run', required=True, help='run.jsonl')
    ap.add_argument('--stderr', default='')
    ap.add_argument('--session-dir', default='')
    ap.add_argument('--tests', default='')
    ap.add_argument('--diff', default='')
    ap.add_argument('--wall', type=float, default=None)
    ap.add_argument('--timeout', default='false')
    ap.add_argument('--exit-code', type=int, default=None)
    ap.add_argument('--arm', default='')
    ap.add_argument('--workload', default='')
    ap.add_argument('--rep', default='')
    ap.add_argument('--attempt', default='1')
    ap.add_argument('--net-path', default='unknown')
    ap.add_argument('--net-tun', default='none')
    ap.add_argument('--loop-guard', default='')
    ap.add_argument('--retry-of', default='')
    ap.add_argument('--ts', default='')
    ap.add_argument('--w1-integrity', default='')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()

    core, lines = scan_jsonl(a.run)
    led = scan_ledger(a.session_dir)
    tests = parse_tests(a.tests) if a.tests else {
        'tests_exit': None, 'tests_last_line': '', 'w1_names_present': [],
        'w1_names_missing': [], 'tests_summary_line': '', 'tests_result_registry': '',
        'tests_passed': None, 'tests_failed': None,
        'base_tests_present': [], 'base_tests_missing': []}
    w1i = {}
    if a.w1_integrity and os.path.exists(a.w1_integrity):
        with open(a.w1_integrity) as fh:
            w1i = json.load(fh)
    diff = parse_diff(a.diff) if a.diff else {
        'diff_files_changed': 0, 'diff_insertions': 0, 'diff_deletions': 0,
        'files_touched': [], 'diff_untracked': []}

    core['usage_match'] = (
        core['total_input_cum'] == core['total_input_cum_turnend']
        and core['total_output_cum'] == core['total_output_cum_turnend'])

    stderr_txt = read_text(a.stderr) if a.stderr else ''
    solpi_ext_errors = sum(
        1 for ln in stderr_txt.splitlines()
        if 'Extension error' in ln and 'SoL-Pi' in ln)

    wall = a.wall
    if wall is None:
        w = read_text(a.run + '.wall').strip()
        wall = float(w) if w else None
    timeout = str(a.timeout).lower() in ('true', '1', 'yes')

    m = {
        'ts': a.ts, 'arm': a.arm, 'workload': a.workload, 'rep': a.rep,
        'pi_exit_code': a.exit_code,
        'wall_seconds': wall, 'timeout': timeout,
        'jsonl_lines': lines,
        'unparsed_lines': core['unparsed_lines'],
        'usage_match': core['usage_match'],
        'models': core['models'],
        'total_input_cum': core['total_input_cum'],
        'total_output_cum': core['total_output_cum'],
        'cache_read_cum': core['cache_read_cum'],
        'cache_write_cum': core['cache_write_cum'],
        'reasoning_cum': core['reasoning_cum'],
        'tool_steps': core['tool_steps'],
        'turns': core['turns'],
        'message_end_events': core['message_end_events'],
        'turn_end_events': core['turn_end_events'],
        'native_compactions': core['compactions'],
        'compactions': core['compactions'] + led['solpi_compactions'],
        'solpi_compactions': led['solpi_compactions'],
        'epr_records': led['epr_records'],
        'net_path': a.net_path,
        'net_tun': a.net_tun,
        'peak_context': core['peak_context'],
        'peak_total_tokens': core['peak_total_tokens'],
        'final_context': core['final_context'],
        'stop_reason_last': core['stop_reason_last'],
        'tool_names': core['tool_names'],
        'af_fired': core['af_fired'],
        'solpi_extension_errors': solpi_ext_errors,
        'solpi_extension_loaded': solpi_ext_errors == 0,
        'obs_recall_calls': core['obs_recall_calls'],
        'op_fired': led['op_fired'],
        'op_placeholder': led['op_placeholder'],
        'op_recall': led['op_recall'],
        'op_full': led['op_full'],
        'op_removed_tokens': led['op_removed_tokens'],
        'op_original_tokens': led['op_original_tokens'],
        'ledger_files': led['ledger_files'],
        'ledger_entries': led['ledger_entries'],
    }
    m.update({k: v for k, v in tests.items()})
    m.update(diff)

    # Валидность прогона по активации ( BRIEF Фаза C)
    # Правило активации (решение владельца): W1 — только AF, т.к. ObservationPack в W1
    # не может сработать по конструкции (каждый cargo test даёт разный вывод → нет
    # идентичного re-send >4KB). В W2 — AF или OP.
    m['activation_rule'] = 'af_only' if a.workload == 'W1' else 'af_or_op'
    if a.arm == 'solpi':
        if m['activation_rule'] == 'af_only':
            m['activation_valid'] = m['af_fired'] > 0
        else:
            m['activation_valid'] = (m['af_fired'] > 0 or m['op_fired'] > 0)
    else:
        m['activation_valid'] = (m['af_fired'] == 0 and m['op_fired'] == 0)
    m['arm_valid'] = bool(
        m['solpi_extension_loaded'] and m['activation_valid']) if a.arm == 'solpi' else True

    # ГЕЙТ ВАЛИДНОСТИ (решение владельца): exit 0 от Pi больше не признак успеха.
    # invalid, если хотя бы одно: stop_reason_last == "error" | total_input_cum == 0
    # | tool_steps == 0 | (timeout == true AND turns < 2).
    reasons = []
    if m['stop_reason_last'] == 'error':
        reasons.append('stop_reason=error')
    if m['total_input_cum'] == 0:
        reasons.append('zero_input')
    if m['tool_steps'] == 0:
        reasons.append('zero_tool_steps')
    if m['timeout'] and m['turns'] < 2:
        reasons.append('timeout_before_content')
    m['attempt'] = int(a.attempt or 1)
    m['retry_of'] = int(a.retry_of) if str(a.retry_of).strip().isdigit() else ''
    loop = None
    if a.loop_guard and os.path.exists(a.loop_guard):
        try:
            loop = json.load(open(a.loop_guard))
        except (OSError, json.JSONDecodeError):
            loop = None
    m['loop_guard_fired'] = bool(loop)
    m['loop_guard'] = (loop or {}).get('trigger', '')
    if loop:
        reasons.append('INVALID_LOOP')
    m['valid'] = not reasons
    m['invalid_reason'] = ','.join(reasons)
    m.update(w1i)
    touched_ok = (diff['files_touched'] == ['crates/stellwerk-registry/src/model.rs'])
    base_intact = bool(w1i.get('base_intact', False))
    w1_present = bool(w1i.get('w1_all_present', False)) or not a.w1_integrity
    m['equivalence_ok'] = bool(
        tests['tests_exit'] == 0
        and not tests['w1_names_missing']
        and not tests['base_tests_missing']
        and tests.get('tests_failed') in (0, None)
        and touched_ok
        and base_intact
        and w1_present
        and not diff['diff_untracked'])

    with open(a.out, 'w') as fh:
        json.dump(m, fh, indent=1, ensure_ascii=False, sort_keys=True)
        fh.write('\n')
    print('metrics -> %s' % a.out)
    for k in ('total_input_cum', 'total_output_cum', 'tool_steps', 'turns',
              'compactions', 'peak_context', 'af_fired', 'op_fired', 'timeout',
              'wall_seconds', 'usage_match', 'activation_valid', 'equivalence_ok',
              'solpi_extension_errors', 'solpi_extension_loaded', 'arm_valid',
              'native_compactions', 'solpi_compactions', 'epr_records',
              'loop_guard_fired', 'loop_guard',
              'net_path', 'net_tun',
              'activation_rule', 'base_intact', 'tests_passed', 'valid',
              'invalid_reason', 'attempt', 'retry_of'):
        print('  %-18s %s' % (k, m.get(k)))


if __name__ == '__main__':
    sys.exit(main())
