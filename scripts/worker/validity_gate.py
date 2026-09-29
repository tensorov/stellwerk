#!/usr/bin/env python3
"""validity_gate.py — exit 0 от pi НЕ означает success.

Правило владельца (F-20): годен ТОЛЬКО объявленный успех. Раньше отвергался
единственный stop=error, поэтому прогон, завершившийся на toolUse, проходил как
годный, тогда как раннер тот же прогон отвергал (F-20, три трассы из пяти).

Valid возможно, только если ВСЕ шесть условий:
  1. stop_reason_last == "stop"    провайдер явно объявил успешную остановку
  2. pi_exit == 0                   процесс завершился без ошибки
  3. timeout == false               таймаут не объявляет успеха
  4. финальный message_end есть: последний — role=assistant И с message.usage
  5. трасса не оборвана: последняя строка — валидный JSON И есть agent_end
     либо agent_settled
  6. loop и budget (пороги в dispatch.conf):
     tool_steps == 0                  (воркер не сделал ни одного действия)
     turns < 2                        (меньше двух ходов — не содержательная работа)
     tool_steps > MAX_TOOL_STEPS      (бюджет шагов исчерпан)
     input_cum > MAX_INPUT_CUM        (бюджет контекста исчерпан)

Отказ называется МАШИННЫМ ТОКЕНОМ в invalid_reason (без пробелов: значение уходит
в metrics.json и читается инструментами), первым сработавший в порядке пакета.
reasons остаётся подробным списком для человека.

Код возврата: 0 — valid, 3 — invalid (packet возвращается, не мержится).
Дополнительно (информационно) проверяет наличие хвоста STATUS/FILES/TEST/EVIDENCE.
"""
import argparse
import json
import re
import sys

TAIL = re.compile(r'^\s*(STATUS|FILES|TEST|EVIDENCE):', re.M)
END_MARKERS = ('agent_end', 'agent_settled')


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
    last_role = None
    last_usage = False
    end_marker = False
    tail_json = True   # последняя непустая строка файла — валидный JSON?
    try:
        with open(a.jsonl, errors='replace') as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    ev = json.loads(line)
                    tail_json = True
                except json.JSONDecodeError:
                    tail_json = False
                    continue
                t = ev.get('type')
                if t in END_MARKERS:
                    end_marker = True
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
                    m = ev.get('message') or {}
                    u2 = m.get('usage')
                    # Решает только ПОСЛЕДНИЙ message_end: ответы ассистента выше
                    # по потоку прогон не закрывают, закрывает последний.
                    last_role = m.get('role')
                    last_usage = isinstance(u2, dict) and bool(u2)
    except OSError as exc:
        print('VALIDITY GATE: jsonl недоступен: %s' % exc, file=sys.stderr)
        print(json.dumps({'valid': False, 'reason': 'jsonl_unreadable'}, indent=1))
        return 3

    timeout = str(a.timeout).lower() in ('true', '1', 'yes')
    over_steps = a.max_tool_steps > 0 and tool_steps > a.max_tool_steps
    over_input = a.max_input_cum > 0 and input_cum > a.max_input_cum
    # Первый сработавший, в порядке пакета. Порядок не косметический: у
    # «негоден» без причины нет формы, а перечисление причин читает инструментарий.
    reason = None
    if timeout:
        # Раньше таймаут отвергал только прогон, умерший ДО первого хода; уход в
        # таймаут на середине работы считался успехом. Таймаут успеха не объявляет.
        reason = 'timeout'
    elif stop == 'error':
        reason = 'error'
    elif stop == 'aborted':
        reason = 'aborted'
    elif stop == 'toolUse':
        # Провайдер остановился посреди работы инструмента: ход не закрыт.
        reason = 'incomplete_tool_use'
    elif stop != 'stop':
        # Причину не объявили (None, maxTokens, …). Тишина — не объявленный успех:
        # молчание провайдера и его согласие неразличимы, если второе не названо.
        reason = 'unreported'
    elif last_role != 'assistant' or not last_usage:
        # Финального ответа ассистента с usage нет — предъявлять нечего, все
        # предыдущие ходы могли оборваться на середине.
        reason = 'no_final_message'
    elif not tail_json or not end_marker:
        # Трасса могла быть отрезана: либо последняя строка не JSON, либо
        # закрывающего маркера агента в потоке нет вовсе.
        reason = 'truncated'
    elif tool_steps == 0 or turns < 2:
        reason = 'loop'
    elif over_steps or over_input:
        reason = 'budget'
    elif a.pi_exit != 0:
        reason = 'nonzero_exit'

    reasons = []
    if stop == 'error':
        reasons.append('stop_reason=error')
    if tool_steps == 0:
        reasons.append('tool_steps=0')
    if turns < 2:
        reasons.append('turns<2')
    if timeout and turns < 2:
        reasons.append('timeout_before_content')
    if over_steps:
        reasons.append('budget:tool_steps=%d>%d' % (tool_steps, a.max_tool_steps))
    if over_input:
        reasons.append('budget:input_cum=%d>%d' % (input_cum, a.max_input_cum))
    if reason and not reasons:
        reasons.append(reason)

    res = {
        'valid': not reasons and reason is None,
        'reasons': reasons,
        'invalid_reason': reason,
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
    print('VALIDITY GATE: %s (pi_exit=%s stop=%s turns=%d steps=%d input_cum=%d%s%s)'
          % ('VALID' if res['valid'] else 'INVALID -> packet возвращается',
             a.pi_exit, stop, turns, tool_steps, input_cum,
             '' if reason is None else ' reason=%s' % reason,
             '' if res['tail_ok'] else ' [хвост STATUS/... не найден]'))
    if reasons:
        print('  причины: %s' % ', '.join(reasons))
    return 0 if res['valid'] else 3


if __name__ == '__main__':
    sys.exit(main())
