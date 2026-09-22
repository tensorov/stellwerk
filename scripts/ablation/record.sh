#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# record.sh VAR TOKENS MODEL [NOTE]
#   VAR    — имя варианта (V0, V1, V2, ..., V6b ...), без табов/переводов строк
#   TOKENS — целое >= 0 (число из счётчика провайдера)
#   MODEL  — фактическая модель активного агента (обязательно, P4)
#   NOTE   — необязательная пометка
# Дописывает строку в .ablation/results/ledger.tsv и пересобирает TABLE.md
# (секция Derived сохраняется и перезаписывается только derive.sh).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

[ $# -ge 3 ] || die "usage: record.sh VAR TOKENS MODEL [NOTE]"

VAR="$1"; TOKENS="$2"; MODEL="$3"; NOTE="${4:--}"

[ -n "$VAR" ]     || die "VAR пуст"
case "$TOKENS" in
  ''|*[!0-9]*) die "TOKENS должен быть неотрицательным целым: '$TOKENS'" ;;
esac
[ -n "$MODEL" ]   || die "MODEL обязателен (P4): фактическая модель активного агента"
case "$VAR" in
  *$'\t'*|*$'\n'*) die "VAR не должен содержать табы/переводы строк" ;;
esac

mkdir -p -- "$RESULTS_ROOT"
ledger="$RESULTS_ROOT/ledger.tsv"
md="$RESULTS_ROOT/TABLE.md"
[ -f "$ledger" ] || printf 'VAR\tTOKENS\tMODEL\tNOTE\tTS\n' > "$ledger"

ts="$(stamp)"
printf '%s\t%s\t%s\t%s\t%s\n' "$VAR" "$TOKENS" "$MODEL" "$NOTE" "$ts" >> "$ledger"

inner="$(derived_inner_current "$md")"
render_table "$ledger" "$md" "$inner"

log "записано: $VAR $TOKENS $MODEL ($NOTE)"
printf '%s\n' "$VAR	$TOKENS	$MODEL	$NOTE	$ts"