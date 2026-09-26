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
        fired = cm > a.consec_limit or (a.total_limit > 0 and tm > a.total_limit)
        print('dry-run: consec_max=%d total_max=%d limit=%d → %s'
              % (cm, tm, a.consec_limit, 'СРАБОТАЕТ' if fired else 'не сработает'))
        if fired and cm > a.consec_limit:
            print('   подряд: %s x%d' % ((wc if isinstance(wc, str) else '-')[:70], cm))
        if fired and a.total_limit > 0 and tm > a.total_limit:
            print('   суммарно: %s x%d' % ((wt if isinstance(wt, str) else '-')[:70], tm))
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
            if consec > a.consec_limit:
                hit = 'consecutive=%d' % consec
            elif a.total_limit > 0 and totals[s] > a.total_limit:
                hit = 'total=%d' % totals[s]
            if hit:
                with open(a.trigger, 'w') as tf:
                    json.dump({'reason': 'FAILED(loop)', 'trigger': hit, 'signature': s[:200],
                               'tool_steps_seen': sum(totals.values()),
                               'consec_limit': a.consec_limit}, tf, indent=1)
                sys.stderr.write('LOOP_GUARD: %s (%s)\n' % (hit, s[:90]))
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
