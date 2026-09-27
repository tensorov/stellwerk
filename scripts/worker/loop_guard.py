#!/usr/bin/env python3
"""loop_guard.py — живое прерывание цикла повторения в воркер-слое.

Порог конфигурируется (LOOP_CONSEC_LIMIT, по умолчанию 20). При N+1 идентичных
tool-вызовах подряд (tool + нормализованные args) — SIGTERM процессу worker'а,
затем SIGKILL через LOOP_KILL_AFTER секунд. Маркер: <run>/loop_guard.json.

Происхождение: .ablation-solpi (SoL-Pi ablation) — qwen3.6 уходил в цикл на
packet-07a (104 идентичных grep подряд из 166 шагов). Требование зафиксировано в
.stellwerk/FINDINGS.md F-2026-09-26-01.
"""
import argparse
import json
import os
import signal
import sys
import time


def signature(obj):
    if obj.get('type') != 'tool_execution_start':
        return None
    args = obj.get('args') or {}
    norm = json.dumps(args, sort_keys=True, ensure_ascii=False, separators=(',', ':'))
    return '%s|%s' % (obj.get('toolName') or '?', ' '.join(norm.split()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--jsonl', required=True)
    ap.add_argument('--trigger', required=True)
    ap.add_argument('--pid', type=int, default=0)
    ap.add_argument('--consec-limit', type=int, default=20)
    ap.add_argument('--total-limit', type=int, default=0)
    ap.add_argument('--kill-after', type=int, default=10)
    ap.add_argument('--interval', type=float, default=1.5)
    # Бюджетные пороги: ловим на лету то, что consec-guard пропускает —
    # длинный прогон с РАЗНЫМИ вызовами (реальный режим отказа qwen в сессии #2).
    ap.add_argument('--max-tool-steps', type=int, default=0)
    ap.add_argument('--max-input-cum', type=int, default=0)
    ap.add_argument('--dry-run', action='store_true')
    a = ap.parse_args()

    if a.dry_run:
        consec = last = 0
        totals = {}
        cm = tm = 0
        wc = wt = None
        with open(a.jsonl, errors='replace') as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                s = signature(obj)
                if s is None:
                    continue
                consec = consec + 1 if s == last else 1
                last = s
                totals[s] = totals.get(s, 0) + 1
                if consec > cm:
                    cm, wc = consec, s
                if totals[s] > tm:
                    tm, wt = totals[s], s
        # В dry-run суммарные шаги считаем по всему файлу, а не по подряд идущим.
        total_steps = 0
        total_input = 0
        with open(a.jsonl, errors='replace') as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    o = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if o.get('type') == 'tool_execution_start':
                    total_steps += 1
                elif o.get('type') == 'turn_end':
                    u = ((o.get('message') or {}).get('usage') or {})
                    if isinstance(u, dict):
                        total_input += int(u.get('input', 0) or 0) + \
                                       int(u.get('cacheRead', 0) or 0) + \
                                       int(u.get('cacheWrite', 0) or 0)
        over_steps = a.max_tool_steps > 0 and total_steps > a.max_tool_steps
        over_input = a.max_input_cum > 0 and total_input > a.max_input_cum
        fired = (cm > a.consec_limit or (a.total_limit > 0 and tm > a.total_limit)
                 or over_steps or over_input)
        if over_steps:
            print('   бюджет шагов: %d > %d' % (total_steps, a.max_tool_steps))
        if over_input:
            print('   бюджет контекста: %d > %d' % (total_input, a.max_input_cum))
        print('dry-run: consec_max=%d total_max=%d limit=%d → %s'
              % (cm, tm, a.consec_limit, 'СРАБОТАЕТ' if fired else 'не сработает'))
        if fired and cm > a.consec_limit:
            print('   подряд: %s x%d' % ((wc if isinstance(wc, str) else '-')[:70], cm))
        if fired and a.total_limit > 0 and tm > a.total_limit:
            print('   суммарно: %s x%d' % ((wt if isinstance(wt, str) else '-')[:70], tm))
        return 0 if fired else 1

    offset = 0
    consec = last = 0
    seen_steps = 0
    input_cum = 0
    totals = {}
    while True:
        time.sleep(a.interval)
        try:
            size = os.path.getsize(a.jsonl)
            if size < offset:
                offset = 0
            if size == offset:
                continue
            with open(a.jsonl, errors='replace') as fh:
                fh.seek(offset)
                buf = fh.read()
                offset = fh.tell()
        except OSError:
            continue
        for line in buf.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            # Учёт usage ВЫНЕСЕН ПЕРЕД ранним continue. turn_end не является
            # вызовом инструмента, поэтому signature() для него даёт None и
            # весь блок ниже был МЁРТВЫМ: live input_cum всегда оставался 0.
            # Как метрика это не проявлялось (её считает postflight), но
            # опция включения обратно была бы ловушкой: включил — молча не
            # срабатывало.
            if obj.get('type') == 'turn_end':
                u = ((obj.get('message') or {}).get('usage') or {})
                if isinstance(u, dict):
                    input_cum += int(u.get('input', 0) or 0) + \
                                 int(u.get('cacheRead', 0) or 0) + \
                                 int(u.get('cacheWrite', 0) or 0)
            s = signature(obj)
            if s is None:
                continue
            seen_steps += 1
            consec = consec + 1 if s == last else 1
            last = s
            totals[s] = totals.get(s, 0) + 1
            hit = None
            # РЕШЕНИЕ ВЛАДЕЛЬЦА: input_cum ВЫВЕДЕН ИЗ УСЛОВИЙ УБИЙСТВА.
            # Данные (tests/traces/MANIFEST.md) показали, что счётчик классы
            # не разделяет: cacheRead — ~80% метрики, у «зависших» трасс
            # 521904/454832 против легитимной 519808 (разница <1%), разброс
            # одной и той же работы 2x, а порог 400000 резал легитимный прогон
            # canary (655622, accept.sh: ПРИНЯТО). От застревания защищают
            # шаги, guard на одинаковые вызовы, wall-clock таймаут и гейты
            # качества. Ветка input_cum сохранена как ЯВНАЯ ОПЦИЯ: работает
            # только при MAX_INPUT_CUM > 0, по умолчанию мертва.
            if a.max_tool_steps > 0 and seen_steps > a.max_tool_steps:
                hit = 'budget:tool_steps=%d>%d' % (seen_steps, a.max_tool_steps)
            elif a.max_input_cum > 0 and input_cum > a.max_input_cum:
                hit = 'budget:input_cum=%d>%d' % (input_cum, a.max_input_cum)
            if consec > a.consec_limit:
                hit = 'consecutive=%d' % consec
            elif a.total_limit > 0 and totals[s] > a.total_limit:
                hit = 'total=%d' % totals[s]
            if hit:
                with open(a.trigger, 'w') as tf:
                    json.dump({'reason': 'FAILED(budget)' if 'budget' in hit else 'FAILED(loop)',
                           'trigger': hit, 'signature': s[:200],
                               'tool_steps_seen': sum(totals.values()),
                               'consec_limit': a.consec_limit}, tf, indent=1)
                sys.stderr.write('LOOP_GUARD: %s (%s)\n' % (hit, s[:90]))
                globals()['_BUDGET'] = hit
                if a.pid:
                    for sig, delay in ((signal.SIGTERM, 0), (signal.SIGKILL, a.kill_after)):
                        if delay:
                            time.sleep(delay)
                        try:
                            os.kill(a.pid, sig)
                        except OSError:
                            pass
                return 0
    return 0


if __name__ == '__main__':
    sys.exit(main())
