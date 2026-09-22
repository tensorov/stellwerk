#!/usr/bin/env python3
# -*- coding: utf-8 -*-
#
# extract-tokens.py <events.ndjson>
#
# Разбирает NDJSON-поток `opencode run --format json` и извлекает token usage.
# Понимает обе конвенции финального шага:
#   * event.part.type == "step-finish"  (StepFinishPart: part.tokens{cache{read,write},...})
#   * event.type    == "step_finish"
# Плюс запасной поиск: любой вложенный usage-объект с числовыми input+cache.read|output.
#
# Главная метрика ablation — first_step.context_input (первый шаг до tool-loops);
# all_steps.* сохраняется как диагностика (не суммируется в решение).
#
# Вывод: один JSON-объект в stdout. warnings (не-JSON строки и пр.) — в stderr.

import json
import os
import sys


def _num(v):
    if isinstance(v, bool):
        return 0
    if isinstance(v, int):
        return v
    if isinstance(v, float):
        return int(v)
    return 0


def _total(v):
    if isinstance(v, bool):
        return None
    if isinstance(v, (int, float)):
        return int(v)
    return None


def looks_usage(d):
    if not isinstance(d, dict):
        return False
    if not isinstance(d.get("input"), (int, float)):
        return False
    if isinstance(d.get("input"), bool):
        return False
    cache = d.get("cache")
    if isinstance(cache, dict) and isinstance(cache.get("read"), (int, float)):
        return True
    if isinstance(d.get("output"), (int, float)) and not isinstance(d.get("output"), bool):
        return True
    return False


def find_usage_objects(node, out):
    seen = set()
    stack = [node]
    while stack:
        cur = stack.pop()
        if isinstance(cur, dict):
            if id(cur) in seen:
                continue
            seen.add(id(cur))
            if looks_usage(cur):
                out.append(cur)
            stack.extend(cur.values())
        elif isinstance(cur, list):
            stack.extend(cur)


def usage_to_record(u):
    cache = u.get("cache") or {}
    rec = {
        "input": _num(u.get("input")),
        "cache_read": _num(cache.get("read") or u.get("cache_read") or u.get("cacheRead")),
        "cache_write": _num(cache.get("write") or u.get("cache_write") or u.get("cacheWrite")),
        "output": _num(u.get("output")),
        "reasoning": _num(u.get("reasoning")),
        "total": _total(u.get("total")),
    }
    rec["context_input"] = rec["input"] + rec["cache_read"] + rec["cache_write"]
    return rec


def extract_from(container):
    for key in ("tokens", "usage"):
        v = container.get(key)
        if looks_usage(v):
            return usage_to_record(v)
    cands = []
    find_usage_objects(container, cands)
    if cands:
        return usage_to_record(cands[0])
    return None


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else None
    fh = open(path, "r", encoding="utf-8", errors="replace") if path else sys.stdin

    steps = []
    session_id = None
    warnings = 0

    for lineno, line in enumerate(fh, 1):
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            warnings += 1
            print(f"[extract-tokens] warning: line {lineno}: non-JSON, ignored", file=sys.stderr)
            continue
        if not isinstance(ev, dict):
            warnings += 1
            print(f"[extract-tokens] warning: line {lineno}: not an object, ignored", file=sys.stderr)
            continue

        sid = ev.get("session_id") or ev.get("sessionID")
        if isinstance(sid, str) and sid:
            session_id = sid

        rec = None
        part = ev.get("part")
        etype = ev.get("type")

        if isinstance(part, dict) and part.get("type") == "step-finish":
            sid2 = part.get("sessionID") or part.get("session_id")
            if isinstance(sid2, str) and sid2:
                session_id = sid2
            rec = extract_from(part)
        elif etype in ("step_finish", "step.finish", "step-finish"):
            rec = extract_from(ev)

        if rec is None and etype is None:
            cands = []
            find_usage_objects(ev, cands)
            if cands:
                rec = usage_to_record(cands[0])

        if rec is not None:
            steps.append(rec)

    fh.close()

    if not steps:
        print(json.dumps({
            "valid": False,
            "session_id": session_id or "",
            "steps": 0,
            "first_step": None,
            "all_steps": None,
            "warnings": warnings,
        }, ensure_ascii=False, indent=2))
        return 1 if not os.environ.get("ABLATION_EXTRACT_OK_ON_EMPTY") else 0

    first = steps[0]
    agg = {
        "input": sum(s["input"] for s in steps),
        "cache_read": sum(s["cache_read"] for s in steps),
        "cache_write": sum(s["cache_write"] for s in steps),
        "output": sum(s["output"] for s in steps),
        "reasoning": sum(s["reasoning"] for s in steps),
        "total": sum(s["total"] for s in steps if s["total"] is not None) or None,
    }
    agg["context_input"] = agg["input"] + agg["cache_read"] + agg["cache_write"]

    summary = {"input": first["input"], "cache_read": first["cache_read"],
               "cache_write": first["cache_write"], "context_input": first["context_input"],
               "output": first["output"], "reasoning": first["reasoning"],
               "total": first["total"]}
    agg_summary = {"input": agg["input"], "cache_read": agg["cache_read"],
                   "cache_write": agg["cache_write"], "context_input": agg["context_input"],
                   "output": agg["output"], "reasoning": agg["reasoning"],
                   "total": agg["total"]}

    print(json.dumps({
        "valid": True,
        "session_id": session_id or "",
        "steps": len(steps),
        "first_step": summary,
        "all_steps": agg_summary,
        "warnings": warnings,
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())