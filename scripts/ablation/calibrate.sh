#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# calibrate.sh — ОДИН замер V0 (чистый runtime) для калибровки метрики.
# Печатает input / cache.read / cache.write / context_input (первый шаг)
# и инструкцию: сравнить один раз context_input с UI/provider-счётчиком и
# зафиксировать ABLATION_METRIC (input или context_input; default context_input).
#
#   MODEL=provider/model ./scripts/ablation/calibrate.sh
#
# Не является измерением для решения: это проверка согласованности счётчиков.
# После замера состояние восстанавливается (restore из LATEST) и
# current-run.env сбрасывается, чтобы run-all стартовал счистого состояния.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

[ -n "${MODEL:-}" ] || die "MODEL не задан: MODEL=provider/model $(basename "$0")"

VAR=V0
REP=cal

# run-one применяет V0 (если ещё не применён), пишет raw и usage.json.
ok="$(MODEL="$MODEL" "$SCRIPT_DIR/run-one.sh" "$VAR" "$REP")"
log "$ok"
rundir="$(printf '%s\n' "$ok" | sed -n 's/.*raw=\(.*\)$/\1/p')"
[ -n "$rundir" ] && [ -f "$rundir/usage.json" ] || die "calibrate: не нашёл usage.json для V0"

V0_USAGE="$rundir/usage.json"

printf '\n== Калибровка V0 (raw: %s) ==\n' "$rundir"
"$PYTHON3" - "$V0_USAGE" <<'PY'
import json, sys
u = json.load(open(sys.argv[1]))
f, a = u["first_step"], u["all_steps"]
print(f"valid            : {u.get('valid')}")
print(f"session_id       : {u.get('session_id') or '-'}")
print(f"steps            : {u.get('steps')}")
print(f"tokens.input     : {f['input']}")
print(f"tokens.cache.read: {f['cache_read']}")
print(f"tokens.cache.write:{f['cache_write']}")
print(f"context_input    : {f['context_input']}   (input + cache.read + cache.write)")
print(f"all_steps.context_input (diagnostics): {a['context_input']}")
print()
print("Сравни ОДИН раз context_input =", f["context_input"], " с UI/provider счётчиком.")
print("  если UI совпадает с input        -> экспортируй ABLATION_METRIC=input")
print("  если UI совпадает с context_input -> ABLATION_METRIC=context_input (default)")
print("Затем запусти: MODEL=provider/model ./scripts/ablation/run-all.sh")
PY

# Вернуть состояние: restore из LATEST (снимок pre-V0 создан run-one при apply)
# и сбросить current-run.env, чтобы run-all стартовал с чистого состояния.
"$SCRIPT_DIR/restore.sh" >/dev/null 2>&1 || die "calibrate: restore LATEST не удался (нет pre-V0 снимка?)"
rm -f -- "$CURRENT_RUN_ENV"
log "calibrate: состояние восстановлено, current-run.env сброшен"
exit 0