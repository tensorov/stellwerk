#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# run-one.sh VAR [REPETITION]
#   Один замер одного варианта: отдельный `opencode run` (fresh process/session).
#
# Применяет вариант, только если он ещё не применён (current-run.env != VAR).
# Каждый замер — НОВЫЙ процесс opencode: никаких --continue/--session/--fork.
# Промпт — ровно ".", модель через $MODEL, агент из current-run.env.
#
# Окружение:
#   MODEL=provider/model          (обязательно)
#   ABLATION_METRIC=input|context_input  (по умолчанию context_input)
#   ABLATION_TIMEOUT_SECONDS=300  (по умолчанию)
#   OPENCODE_BIN=opencode         (для QA подменяется фиксурой)
#
# Артефакты: .ablation/results/raw/<VAR>/<ts>-r<N>/:
#   events.ndjson  stderr.log  exit-code.txt  usage.json  apply.log
# Строка в расширенном TSV: .ablation/results/raw/<VAR>/results.tsv
# При успехе — строка record.sh <VAR>-r<N> <metric>.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

[ -n "${MODEL:-}" ] || die "MODEL не задан: MODEL=provider/model $(basename "$0") $*"
VAR="${1:-}"
REP="${2:-1}"
case "$VAR" in
  V[0-9]|V5h|V5x|V6b) ;;
  *) die "неизвестный VAR: '$VAR' (ожидается V0..V9/V5h/V5x/V6b)" ;;
esac
case "$REP" in
  ''|*[!A-Za-z0-9]*|*-*) die "некорректное REPETITION: '$REP'" ;;
esac
ABLATION_METRIC="${ABLATION_METRIC:-context_input}"
export ABLATION_METRIC
case "$ABLATION_METRIC" in
  input|context_input) ;;
  *) die "ABLATION_METRIC должен быть input|context_input, получено: '$ABLATION_METRIC'" ;;
esac
TIMEOUT="${ABLATION_TIMEOUT_SECONDS:-300}"
[ "$TIMEOUT" -eq "$TIMEOUT" ] 2>/dev/null || die "ABLATION_TIMEOUT_SECONDS должен быть целым"

RAW_ROOT="$RESULTS_ROOT/raw/$VAR"
mkdir -p -- "$RAW_ROOT"
stamp_ts="$(stamp)"
rundir="$RAW_ROOT/${stamp_ts}-r${REP}"
mkdir -p -- "$rundir"

# --- применяем вариант, если ещё не применён ---
read_current_run_env
if [ "$VAR" = V6b ]; then
  if [ "$CUR_VAR" != "V6" ]; then
    die "V6b требует применённый V6; current-run.env VAR='$CUR_VAR'"
  fi
  AGENT="$CUR_AGENT"; XDG="$CUR_XDG"; CUR_NOTE="$CUR_NOTE"
  CUR_VAR="V6b"
elif [ "$CUR_VAR" != "$VAR" ]; then
  [ -f "$ABL_SCRIPT_DIR/apply-$VAR.sh" ] || die "нет apply-$VAR.sh для $VAR"
  log "применяю $VAR (current-run.env='${CUR_VAR:-<нет>}')"
  bash "$ABL_SCRIPT_DIR/apply-$VAR.sh" >"$rundir/apply.log" 2>&1 || {
    cat "$rundir/apply.log" >&2
    die "apply-$VAR.sh не отработал"
  }
  read_current_run_env
fi
[ -f "$CURRENT_RUN_ENV" ] || die "нет current-run.env после apply-$VAR"
[ "$CUR_VAR" = "$VAR" ] || die "current-run.env VAR='$CUR_VAR', ждали '$VAR'"
AGENT="${CUR_AGENT:-}"
[ -n "$AGENT" ] || die "current-run.env не задаёт AGENT для $VAR"
XDG="${CUR_XDG:-}"
NOTE="${CUR_NOTE:-}"

log "замер: VAR=$VAR rep=$REP agent=$AGENT model=$MODEL xdg='${XDG:-<нет>}' timeout=${TIMEOUT}s"

# --- запуск одного свежего opencode run (timeout-обёртка) ---
# `if ... then ec=0; else ec=$?` защищает ненулевой exit (INVALID/таймаут)
# от срабатывания set -e.
if [ -n "$XDG" ]; then
  if ( cd "$REPO_ROOT" && env XDG_CONFIG_HOME="$XDG" \
        timeout -k 10 "$TIMEOUT" "$OPENCODE_BIN" run --format json --model "$MODEL" --agent "$AGENT" \
        --title "stellwerk-ablation-${VAR}-r${REP}" "." ) \
      >"$rundir/events.ndjson" 2>"$rundir/stderr.log"; then
    ec=0
  else
    ec=$?
  fi
else
  if ( cd "$REPO_ROOT" && \
        timeout -k 10 "$TIMEOUT" "$OPENCODE_BIN" run --format json --model "$MODEL" --agent "$AGENT" \
        --title "stellwerk-ablation-${VAR}-r${REP}" "." ) \
      >"$rundir/events.ndjson" 2>"$rundir/stderr.log"; then
    ec=0
  else
    ec=$?
  fi
fi
printf '%s\n' "$ec" > "$rundir/exit-code.txt"

# --- извлечение usage ---
"$PYTHON3" "$ABL_SCRIPT_DIR/extract-tokens.py" "$rundir/events.ndjson" \
  > "$rundir/usage.json" 2> "$rundir/extract-warnings.log" || {
  # valid=false сам по себе — не ошибка парсера (нет step_finish)
  :
}

# --- классификация ---
status=INVALID
metric_val=""
if [ "$ec" -eq 124 ]; then
  status=TIMEOUT
  log "TIMEOUT: $VAR-r$REP (${TIMEOUT}s истёк)"
elif [ "$ec" -ne 0 ]; then
  status=INVALID
  log "INVALID: $VAR-r$REP exit=$ec"
elif ! "$PYTHON3" -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1])).get("valid") else 1)' "$rundir/usage.json"; then
  status=INVALID
  log "INVALID: $VAR-r$REP нет step_finish (usage.valid=false), exit=0"
else
  metric_val="$("$PYTHON3" - "$rundir/usage.json" <<PY
import json, os, sys
u = json.load(open(sys.argv[1]))
m = os.environ["ABLATION_METRIC"]
if m == "input":
    print(u["first_step"]["input"])
else:
    print(u["first_step"]["context_input"])
PY
)"
  status=OK
  log "OK: $VAR-r$REP ${ABLATION_METRIC}=$metric_val (exit=0, steps=$("$PYTHON3" -c 'import json,sys;print(json.load(open(sys.argv[1]))["steps"])' "$rundir/usage.json"), session=$("$PYTHON3" -c 'import json,sys;print(json.load(open(sys.argv[1]))["session_id"] or "-")' "$rundir/usage.json"))"
fi

# --- расширенный TSV ---
tsv="$RAW_ROOT/results.tsv"
if [ ! -f "$tsv" ]; then
  printf 'timestamp\tvar\trepetition\tmodel\tagent\tinput\tcache_read\tcache_write\tcontext_input\toutput\treasoning\tall_steps_context_input\tsession_id\texit_code\tstatus\n' > "$tsv"
fi
ROW_TS="$(stamp)" ROW_VAR="$VAR" ROW_REP="$REP" MODEL="$MODEL" ROW_AGENT="$AGENT" \
  ROW_EXIT="$ec" STATUS="$status" \
  "$PYTHON3" - "$rundir/usage.json" "$tsv" <<'PY'
import json, os, sys

env = os.environ
usage = {}
try:
    with open(sys.argv[1]) as fh:
        usage = json.load(fh)
except Exception:
    usage = {}
f = usage.get("first_step") or {}
a = usage.get("all_steps") or {}
row = [
    env["ROW_TS"], env["ROW_VAR"], env["ROW_REP"], env.get("MODEL", ""), env.get("ROW_AGENT", ""),
    f.get("input", 0), f.get("cache_read", 0), f.get("cache_write", 0), f.get("context_input", 0),
    f.get("output", 0), f.get("reasoning", 0), a.get("context_input", 0),
    usage.get("session_id", "") or "", env["ROW_EXIT"], env["STATUS"],
]
with open(sys.argv[2], "a") as fh:
    fh.write("\t".join(str(x) for x in row) + "\n")
PY

# --- record.sh при успехе (без выдуманных чисел при INVALID/TIMEOUT) ---
if [ "$status" = OK ]; then
  bash "$SCRIPT_DIR/record.sh" "${VAR}-r${REP}" "$metric_val" "$MODEL" \
    "auto rep${REP}/3: agent=$AGENT ${ABLATION_METRIC}=first_step.${ABLATION_METRIC}; run $rundir"
else
  log "$VAR-r$REP: статус $status — в ledger ничего не записано (без выдуманных токенов)"
fi

printf 'run-one %s-r%s status=%s metric=%s raw=%s\n' "$VAR" "$REP" "$status" "${metric_val:--}" "$rundir"
exit 0