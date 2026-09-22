#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# run-all.sh — полный автоматический прогон ablation:
#   restore(root) -> verify(root) -> apply(примерно first rep) ->
#   run-one 3 раза -> restore(root) -> verify(root)  для V0 V1 V2 V4 V5 V5h V5x,
#   V6 -> V6 -> V6b (3+3 новых сессии), V7, V9 → финал restore+verify+just doctor+derive.
#
#   MODEL=provider/model ./scripts/ablation/run-all.sh
#
# Окружение:
#   MODEL=provider/model          (обязательно, не хардкодится)
#   ABLATION_METRIC=input|context_input (по умолчанию context_input)
#   ABLATION_TIMEOUT_SECONDS=300  (по умолчанию)
#   OPENCODE_BIN=opencode         (для QA подменяется фиксурой)
#
# Гарантии:
#   * корневой снапшот pre-ablation-auto до старта; trap восстанавливает его
#     на любом exit/signal;
#   * каждый замер = отдельный `opencode run` (fresh process, без serve);
#   * между вариантами restore+verify;
#   * final: restore(root) + verify(root) + just doctor + derive.sh;
#   * SUMMARY.md в .ablation/results/runs/<run-id>/.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

[ -n "${MODEL:-}" ] || die "MODEL не задан: MODEL=provider/model $(basename "$0")"
ABLATION_METRIC="${ABLATION_METRIC:-context_input}"
export ABLATION_METRIC
case "$ABLATION_METRIC" in
  input|context_input) ;;
  *) die "ABLATION_METRIC должен быть input|context_input, получено: '$ABLATION_METRIC'" ;;
esac
TIMEOUT="${ABLATION_TIMEOUT_SECONDS:-300}"
export ABLATION_TIMEOUT_SECONDS

# --- 1) чистота ablation-state ---
shopt -s nullglob
stray=("$REPO_ROOT"/*.ablation_bak_* "$REPO_ROOT"/.opencode.ablation_bak_* "$REPO_ROOT"/opencode.json.ablation_bak_*)
shopt -u nullglob
if [ "${#stray[@]}" -gt 0 ]; then
  die "грязное состояние: найдены *.ablation_bak_* — сначала restore: ${stray[*]}"
fi

# --- 2) корневой снапшот ---
ROOT_SNAP="$("$SCRIPT_DIR/snapshot.sh" "pre-ablation-auto")"
log "корневой снапшот: $ROOT_SNAP"

cleanup() {
  local ec=$?
  trap - EXIT INT TERM
  log "run-all cleanup (exit=$ec): restore корневого снапшота"
  "$SCRIPT_DIR/restore.sh" "$ROOT_SNAP" >/dev/null 2>&1 || \
    log "run-all: restore при trap не удался"
  "$SCRIPT_DIR/verify.sh" "$ROOT_SNAP" >/dev/null 2>&1 || \
    log "run-all: verify после trap — расхождения есть (смотри stderr)"
  rm -f -- "$CURRENT_RUN_ENV"
  exit "$ec"
}
trap cleanup EXIT INT TERM

# --- 3) run-id и метаданные ---
run_id="$(stamp)"
RUN_DIR="$RESULTS_ROOT/runs/$run_id"
mkdir -p -- "$RUN_DIR"
METADATA_FILE="$RUN_DIR/run-metadata.tsv"

opencode_ver="$("$OPENCODE_BIN" --version 2>/dev/null || echo '?')"
omo_ver="?"
omo_pkg="${ABL_HOME}/.cache/opencode/packages/oh-my-openagent@latest/package.json"
if [ -f "$omo_pkg" ]; then
  omo_ver="$("$PYTHON3" - "$omo_pkg" <<'PY'
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("version", "?"))
except Exception:
    print("?")
PY
)"
fi
git_head="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo '?')"
{
  printf 'start_utc\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'opencode_version\t%s\n' "$opencode_ver"
  printf 'omo_version\t%s\n' "$omo_ver"
  printf 'model\t%s\n' "$MODEL"
  printf 'ablation_metric\t%s\n' "$ABLATION_METRIC"
  printf 'timeout_seconds\t%s\n' "$TIMEOUT"
  printf 'git_head\t%s\n' "$git_head"
  printf 'hostname\t%s\n' "$(hostname 2>/dev/null || echo '?')"
  printf 'uname\t%s\n' "$(uname -srm 2>/dev/null || echo '?')"
  printf 'root_snapshot\t%s\n' "$ROOT_SNAP"
  printf 'opencode_bin\t%s\n' "$OPENCODE_BIN"
} | tee "$METADATA_FILE" | while IFS=$'\t' read -r k v; do
  log "meta $k: $v"
done

run_one() { # VAR REP
  local v="$1" r="$2"
  "$SCRIPT_DIR/run-one.sh" "$v" "$r"
}

restore_verify() {
  "$SCRIPT_DIR/restore.sh" "$ROOT_SNAP" >/dev/null 2>&1
  "$SCRIPT_DIR/verify.sh" "$ROOT_SNAP" >/dev/null 2>&1
  log "root verify OK: $ROOT_SNAP"
}

# --- этапы ---
stages=(V0 V1 V2 V4 V5 V5h V5x V6 V7 V9)
log "этапы: ${stages[*]} (V6b идёт сразу после V6)"
for v in "${stages[@]}"; do
  log "== этап $v: restore root -> verify -> (3 замера) =="
  restore_verify
  for r in 1 2 3; do
    run_one "$v" "$r"
  done
  if [ "$v" = V6 ]; then
    log "== этап V6b: те же V6-конфиги, 3 НОВЫХ процесса (без apply/restore) =="
    for r in 1 2 3; do
      run_one V6b "$r"
    done
  fi
  restore_verify
  log "== этап $v завершён =="
done

# --- агрегаты (медиана 3 повторов) и стабильность ---
SUMMARY="$RUN_DIR/SUMMARY.md"
{
  printf '# Ablation run %s\n\n' "$run_id"
  printf 'Команда: `MODEL=%s ABLATION_METRIC=%s %s/run-all.sh`\n\n' "$MODEL" "$ABLATION_METRIC" "$ABL_SCRIPT_DIR"
  printf '## Метаданные\n\n```\n'
  cat "$METADATA_FILE"
  printf '```\n\n## Варианты: медиана по %s (стабильность = разброс max-min vs 2%% медианы)\n\n' "$ABLATION_METRIC"
  printf '| VAR | OK повторов | median | min | max | spread%% | status |\n'
  printf '|---|--:|---:|---:|---:|---:|---|\n'
} > "$SUMMARY"

for v in V0 V1 V2 V4 V5 V5h V5x V6 V6b V7 V9; do
  tsv="$RESULTS_ROOT/raw/$v/results.tsv"
  [ -f "$tsv" ] || { printf '| %s | 0 | - | - | - | - | NOT_RUN |\n' "$v" >> "$SUMMARY"; continue; }
  mcol=9; [ "$ABLATION_METRIC" = input ] && mcol=6
  values="$(awk -F'\t' -v m="$mcol" -v vv="$v" '$2==vv && $NF=="OK"{print $m}' "$tsv")"
  n_ok="$(printf '%s\n' "$values" | sed '/^$/d' | wc -l)"
  if [ "$n_ok" -eq 0 ]; then
    printf '| %s | 0 | - | - | - | - | NO_OK_REPS |\n' "$v" >> "$SUMMARY"
    continue
  fi
  med="$(median_of $values)"
  lo="$(printf '%s\n' $values | sort -n | head -1)"
  hi="$(printf '%s\n' $values | sort -n | tail -1)"
  sp="$(spread_pct $values)"
  st="$(stability_for "$n_ok" "$sp")"
  printf '| %s | %s | %s | %s | %s | %s | %s |\n' "$v" "$n_ok" "$med" "$lo" "$hi" "$sp" "$st" >> "$SUMMARY"
  # агрегат в ledger (только медиана); note несёт sample-size и статус
  if [ "$n_ok" -eq 3 ]; then
    bash "$SCRIPT_DIR/record.sh" "$v" "$med" "$MODEL" \
      "auto median of 3 (${ABLATION_METRIC}=first_step): stable=$st range=[$lo,$hi]"
  else
    bash "$SCRIPT_DIR/record.sh" "$v" "$med" "$MODEL" \
      "auto median of $n_ok/3 OK (${ABLATION_METRIC}=first_step): НЕПОЛНЫЙ набор, стабильность не подтверждена ($st)"
  fi
done

# --- финал: restore + verify + just doctor + derive ---
restore_verify
{
  printf '\n## Финал\n\n- restore root: `%s`\n- verify root: OK\n\n' "$ROOT_SNAP"
} >> "$SUMMARY"

log "just doctor..."
just_ec=0
if command -v just >/dev/null 2>&1 && [ -f "$REPO_ROOT/justfile" ]; then
  ( cd "$REPO_ROOT" && just doctor 2>&1 | sed 's/^/    /' ) | tee -a "$SUMMARY" || just_ec=$?
else
  printf '    just doctor не выполнен (нет just или justfile)\n' | tee -a "$SUMMARY"
  just_ec=127
fi
printf 'just doctor exit=%s\n' "$just_ec" | tee -a "$SUMMARY"

log "derive.sh..."
{
  printf '\n## Derived / решение (derive.sh)\n\n'
  printf '```\n'
  "$SCRIPT_DIR/derive.sh" 2>&1
  printf '```\n'
} >> "$SUMMARY"

log "SUMMARY: $SUMMARY"
log "ВНИМАНИЕ: если у варианта status UNSTABLE — PASS/FAIL не принимать без анализа raw events."

trap - EXIT INT TERM
restore_verify
rm -f -- "$CURRENT_RUN_ENV"
log "current-run.env сброшен"
log "run-all завершён успешно: $SUMMARY"