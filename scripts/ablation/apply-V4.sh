#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V4.sh — OMO diet #1: disabled_agents max.
# Отключаются все канонические агенты КРОМЕ sisyphus, плюс build и plan.
# Если старт сломается из-за build/plan — убрать их из списка в omo_v4.jsonc
# и отметить в note (это и есть результат эксперимента, не ошибка харнесса).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V4

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v4.jsonc"

emit_current_run_env "$VAR" "sisyphus" \
  "agents diet max (10 OMO + build + plan выключены); sisyphus активен"

print_guide "sisyphus" "$VAR" "agents diet max (10 OMO + build + plan выключены); sisyphus активен"