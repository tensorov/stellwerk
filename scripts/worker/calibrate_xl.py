#!/usr/bin/env python3
"""calibrate_xl.py — калибровка MAX_PEAK_CONTEXT по отчёту одного прогона XL.

Вход — report.json одного прогона (JSON-документ или JSONL событий). Скрипт не
ходит в сеть и не читает ~/.pi. Отсутствующее поле — отказ, а не ноль: неизвестное
не должно выглядеть как замеренное (AGENTS.md, правило 6).

Порог вычисляется формулой, а не зашит:
    MAX_PEAK_CONTEXT = ceiling − max_output − int((ceiling − max_output) * 0.10)
Для XL: 138336 − 8000 − 13033 = 117303.
"""
import argparse
import json
import sys

# Ход в потоке — только turn_end: message_update идёт на каждый дельта-чанк
# (1289 на 10 ходов), message_end приходит с пустым usage.
TURN_EVENT = "turn_end"
XL_CEILING = 138_336             # потолок XL — решение владельца, не измерение
XL_OUTPUT_CEILING = 8_000        # потолок вывода модели (maxTokens), НЕ наблюдённые 365
RESERVE_FRACTION = 0.10          # запас от остатка после вычета потолка вывода
EXPECTED_XL_THRESHOLD = 117_303  # 138336 − 8000 − int(130336 * 0.10)
FIELDS = ("overhead_empty", "max_output", "peak_context", "peak_at_turn",
          "turns", "tool_steps", "wall_time")


class CalibrationError(RuntimeError):
    """Измерение не состоялось; причина обязана быть названа."""


def _as_turn(event):
    if "output" in event:
        return event
    message = event.get("message")
    if isinstance(message, dict) and isinstance(message.get("usage"), dict):
        return message["usage"]
    return event


def _turns(payload):
    events = payload.get("events")
    if isinstance(events, list):
        picked = [e for e in events if isinstance(e, dict) and e.get("type") == TURN_EVENT]
        if not picked:
            raise CalibrationError("в потоке нет ни одного события turn_end")
        return [_as_turn(e) for e in picked]
    usage = payload.get("usage", payload)
    if isinstance(usage, dict):
        for key in ("turns", "turn_end", "messages"):
            value = usage.get(key)
            if isinstance(value, list):
                return [_as_turn(t) for t in value if isinstance(t, dict)]
    if isinstance(usage, list):
        return [_as_turn(t) for t in usage if isinstance(t, dict)]
    raise CalibrationError("отчёт не содержит ни потока событий, ни списка ходов")


def _context_of(turn):
    total = 0
    for key in ("input", "cacheRead", "cacheWrite"):
        value = turn.get(key)
        if not isinstance(value, int):
            return None
        total += value
    return total


def measure(payload):
    turns = _turns(payload)
    if not turns:
        raise CalibrationError("в отчёте нет ни одного хода сессии")
    outputs, contexts = [], []
    for turn in turns:
        value = turn.get("output")
        if not isinstance(value, int):
            raise CalibrationError("поле output отсутствует или не целое хотя бы в одном ходе")
        outputs.append(value)
        size = _context_of(turn)
        if size is None:
            raise CalibrationError("контекст не измерен хотя бы в одном ходе")
        contexts.append(size)
    peak = max(contexts)
    wall = payload.get("wall_time", payload.get("wall_clock_seconds"))
    if not isinstance(wall, (int, float)):
        raise CalibrationError("поле wall_time отсутствует или не число")
    steps = payload.get("tool_steps")
    if steps is None:
        steps = sum(1 for e in payload.get("events", [])
                    if isinstance(e, dict) and e.get("type") == "tool_execution_end")
        if not steps:
            raise CalibrationError("ни поля tool_steps, ни событий tool_execution_end: шаги не измерены")
    if not isinstance(steps, int):
        raise CalibrationError("поле tool_steps не целое")
    return {"overhead_empty": contexts[0], "max_output": max(outputs),
            "peak_context": peak, "peak_at_turn": contexts.index(peak) + 1,
            "turns": len(turns), "tool_steps": steps, "wall_time": wall}


def threshold(ceiling, max_output):
    """MAX_PEAK_CONTEXT = потолок − max_output − 10% остатка. Число вычисляется."""
    if max_output <= 0:
        raise CalibrationError("max_output должен быть положительным")
    reserve = int((ceiling - max_output) * RESERVE_FRACTION)
    value = int(ceiling - max_output - reserve)
    if value <= 0:
        raise CalibrationError("порог неположителен: потолок меньше max_output плюс запас")
    return value


def self_test():
    good = {"wall_time": 812.5, "tool_steps": 41, "usage": {"turns": [
        {"input": 1200, "cacheRead": 0, "cacheWrite": 0, "output": 900},
        {"input": 8000, "cacheRead": 50000, "cacheWrite": 12000, "output": 4000},
    ]}}
    got = measure(good)
    assert set(got) == set(FIELDS), got
    assert (got["max_output"], got["overhead_empty"], got["peak_context"],
            got["peak_at_turn"], got["turns"], got["tool_steps"],
            got["wall_time"]) == (4000, 1200, 70000, 2, 2, 41, 812.5), got
    computed = threshold(XL_CEILING, XL_OUTPUT_CEILING)
    if computed != EXPECTED_XL_THRESHOLD:
        print(f"ПРОВАЛ: формула дала {computed}, ожидалось {EXPECTED_XL_THRESHOLD} "
              f"(потолок {XL_CEILING}, max_output 8000)", file=sys.stderr)
        return 1
    broken = [
        ({**good, "usage": {"turns": []}}, "нет ходов"),
        ({**good, "tool_steps": None}, "нет tool_steps"),
        ({**good, "wall_time": None}, "нет wall_time"),
        ({**good, "usage": {"turns": [
            {"input": 1, "cacheRead": 2, "cacheWrite": 3, "output": 1},
            {"input": 9, "cacheRead": 9, "output": 1}]}}, "неполный набор счётчиков"),
        ({**good, "usage": {"turns": [
            {"input": 1, "cacheRead": 2, "cacheWrite": 3, "output": None}]}},
         "output отсутствует"),
    ]
    for payload, why in broken:
        try:
            measure(payload)
        except CalibrationError:
            continue
        print(f"ПРОВАЛ: {why} принят как измерение", file=sys.stderr)
        return 1
    try:
        threshold(1000, 2000)
    except CalibrationError:
        pass
    else:
        print("ПРОВАЛ: потолок меньше max_output принят", file=sys.stderr)
        return 1
    print("calibrate_xl: self-test OK")
    return 0


def _load(path):
    with open(path, encoding="utf-8") as handle:
        raw = handle.read()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        try:
            events = [json.loads(line) for line in raw.splitlines() if line.strip()]
        except json.JSONDecodeError as exc:
            raise CalibrationError(f"строка не разобрана как JSON: {exc}") from exc
        if not events:
            raise CalibrationError("отчёт пуст")
        return {"events": events}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("report", nargs="?", help="JSON-отчёт прогона")
    ap.add_argument("--model", help="идентификатор модели, для отчёта")
    ap.add_argument("--ceiling", type=int, default=XL_CEILING, help="потолок контекста")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return self_test()
    if not args.report:
        ap.error("нужен путь к отчёту или --self-test")
    try:
        payload = _load(args.report)
        got = measure(payload)
        limit = threshold(args.ceiling, XL_OUTPUT_CEILING)
    except (OSError, CalibrationError) as exc:
        print(f"отказ: {exc}", file=sys.stderr)
        return 1
    out = {"model": args.model, "ceiling": args.ceiling,
           "reserve_fraction": RESERVE_FRACTION, **got, "MAX_PEAK_CONTEXT": limit}
    print(json.dumps(out, ensure_ascii=False, indent=2))
    if got["peak_context"] > limit:
        print(f"внимание: измеренный peak {got['peak_context']} выше порога {limit}",
              file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
