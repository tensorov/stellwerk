#!/usr/bin/env python3
"""validity_gate.py — exit 0 от pi НЕ означает success.

invalid, если хотя бы одно (пороги в dispatch.conf):
  stop_reason_last == "error"          (провайдерский/рантайм сбой)
  tool_steps == 0                      (воркер не сделал ни одного действия)
  turns < 2                            (меньше двух ходов — не содержательная работа)
  timeout == true и turns < 2          (умер до первого содержательного хода)
  tool_steps > MAX_TOOL_STEPS          (бюджет шагов исчерпан)
  input_cum > MAX_INPUT_CUM             (бюджет контекста исчерпан)

Код возврата: 0 — valid, 3 — invalid (packet возвращается, не мержится).
Дополнительно (информационно) проверяет наличие хвоста STATUS/FILES/TEST/EVIDENCE.
"""
import argparse
import json
import re
import sys

TAIL = re.compile(r'^\s*(STATUS|FILES|TEST|EVIDENCE):', re.M)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--jsonl', required=True)
    ap.add_argument('--out', default='')
    ap.add_argument('--pi-exit', type=int, default=0)
    ap.add_argument('--timeout', default='false')
    # Бюджетные пороги (0 = выключено). Происхождение: сессия #2, 3 из 10
    # прогонов ушли в повтор на 480–790k input при окне 46112 — это ~17 полных
    # контекстов подряд, и ни consec-guard, ни пост-фактум это не ловили.
    ap.add_argument('--max-tool-steps', type=int, default=0)
    ap.add_argument('--max-input-cum', type=int, default=0)
    a = ap.parse_args()

    turns = tool_steps = 0
    stop = None
    input_cum = 0
    text = []
    try:
        with open(a.jsonl, errors='replace') as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue
                t = ev.get('type')
                if t == 'tool_execution_start':
                    tool_steps += 1
                elif t == 'turn_end':
                    msg = ev.get('message') or {}
                    if msg.get('stopReason'):
                        stop = msg.get('stopReason')
                    u = msg.get('usage') or {}
                    if isinstance(u, dict):
                        input_cum += int(u.get('input', 0) or 0) + \
                                    int(u.get('cacheRead', 0) or 0) + \
                                    int(u.get('cacheWrite', 0) or 0)
                    c = msg.get('content')
                    if isinstance(c, list):
                        for b in c:
                            if isinstance(b, dict) and b.get('type') == 'text':
                                text.append(b.get('text') or '')
                elif t == 'message_end':
                    turns += 1
    except OSError as exc:
        print('VALIDITY GATE: jsonl недоступен: %s' % exc, file=sys.stderr)
        print(json.dumps({'valid': False, 'reason': 'jsonl_unreadable'}, indent=1))
        return 3

    timeout = str(a.timeout).lower() in ('true', '1', 'yes')
    reasons = []
    if stop == 'error':
        reasons.append('stop_reason=error')
    if tool_steps == 0:
        reasons.append('tool_steps=0')
    if turns < 2:
        reasons.append('turns<2')
    if timeout and turns < 2:
        reasons.append('timeout_before_content')
    if a.max_tool_steps > 0 and tool_steps > a.max_tool_steps:
        reasons.append('budget:tool_steps=%d>%d' % (tool_steps, a.max_tool_steps))
    if a.max_input_cum > 0 and input_cum > a.max_input_cum:
        reasons.append('budget:input_cum=%d>%d' % (input_cum, a.max_input_cum))

    res = {
        'valid': not reasons,
        'reasons': reasons,
        'pi_exit': a.pi_exit,
        'stop_reason_last': stop,
        'turns': turns,
        'tool_steps': tool_steps,
        'timeout': timeout,
        'total_input_cum': input_cum,
        'max_tool_steps': a.max_tool_steps,
        'max_input_cum': a.max_input_cum,
        'tail_ok': bool(TAIL.search('\n'.join(text))),
    }
    if a.out:
        with open(a.out, 'w') as fh:
            json.dump(res, fh, indent=1, ensure_ascii=False)
    print('VALIDITY GATE: %s (pi_exit=%s stop=%s turns=%d steps=%d input_cum=%d%s)'
          % ('VALID' if res['valid'] else 'INVALID -> packet возвращается',
             a.pi_exit, stop, turns, tool_steps, input_cum,
             '' if res['tail_ok'] else ' [хвост STATUS/... не найден]'))
    if reasons:
        print('  причины: %s' % ', '.join(reasons))
    return 0 if res['valid'] else 3


if __name__ == '__main__':
    sys.exit(main())
