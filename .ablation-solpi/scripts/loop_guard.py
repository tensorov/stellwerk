#!/usr/bin/env python3
"""loop_guard.py — детектор цикла повторения тулов, SIGTERM прогону.

Правило владельца: >8 идентичных tool-вызовов ПОДРЯД или >20 идентичных СУММАРНО
(сравнение по tool + нормализованные args) → SIGTERM прогону, метка INVALID_LOOP.

Работает как сторож: хвостит run.jsonl, считает подписи (toolName + args с
отсортированными ключами и схлопнутыми пробелами), при срабатывании пишет
маркер и шлёт SIGTERM процессу timeout, который транслирует его ребёнку (pi).
Через N секунд — SIGKILL.

--dry-run: только анализ существующего JSONL, ничего не убивать (для проверки).
"""
import argparse
import json
import os
import signal
import sys
import time

CONSEC_LIMIT = 8
TOTAL_LIMIT = 20


def signature(obj):
    if obj.get('type') != 'tool_execution_start':
        return None
    args = obj.get('args') or {}
    norm = json.dumps(args, sort_keys=True, ensure_ascii=False, separators=(',', ':'))
    return '%s|%s' % (obj.get('toolName') or '?', ' '.join(norm.split()))


def scan(path):
    """Возвращает (consec_max, total_max, worst_consec_sig, worst_total_sig)."""
    consec = last = 0
    totals = {}
    consec_max = total_max = 0
    worst_consec = worst_total = None
    with open(path, errors='replace') as fh:
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
            if consec > consec_max:
                consec_max, worst_consec = consec, s
            if totals[s] > total_max:
                total_max, worst_total = totals[s], s
    return consec_max, total_max, worst_consec, worst_total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--jsonl', required=True)
    ap.add_argument('--trigger', required=True)
    ap.add_argument('--pid', type=int, default=0)
    ap.add_argument('--kill-after', type=int, default=10)
    ap.add_argument('--interval', type=float, default=1.5)
    ap.add_argument('--dry-run', action='store_true')
    a = ap.parse_args()

    if a.dry_run:
        c, t, wc, wt = scan(a.jsonl)
        fired = c > CONSEC_LIMIT or t > TOTAL_LIMIT
        print('dry-run: consec_max=%d total_max=%d → %s' % (c, t, 'СРАБОТАЕТ' if fired else 'не сработает'))
        if fired:
            wc_s: str = wc if isinstance(wc, str) else '-'
            wt_s: str = wt if isinstance(wt, str) else '-'
            if c > CONSEC_LIMIT:
                print('   по последовательности: %s x%d' % (wc_s[:70], c))
            if t > TOTAL_LIMIT:
                print('   по сумме: %s x%d' % (wt_s[:70], t))
        return 0 if fired else 1

    offset = 0
    consec = last = 0
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
            s = signature(obj)
            if s is None:
                continue
            consec = consec + 1 if s == last else 1
            last = s
            totals[s] = totals.get(s, 0) + 1
            hit = None
            if consec > CONSEC_LIMIT:
                hit = 'consecutive=%d' % consec
            elif totals[s] > TOTAL_LIMIT:
                hit = 'total=%d' % totals[s]
            if hit:
                with open(a.trigger, 'w') as tf:
                    json.dump({'reason': 'INVALID_LOOP', 'trigger': hit,
                               'signature': s[:200],
                               'tool_steps_seen': sum(totals.values())}, tf, indent=1)
                sys.stderr.write('LOOP-GUARD: %s (%s)\n' % (hit, s[:90]))
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
