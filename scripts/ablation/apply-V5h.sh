#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V5h.sh — V5 + disabled_hooks ПОЛНЫЙ HookNameSchema (56 имён,
# сверен с bundle: enum == все runtime isHookEnabled). Отключение части хуков
# (codegraph-bootstrap и др.) может менять поведение на запуске — это diet, ожидаемо.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V5h

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v5h.jsonc"

emit_current_run_env "$VAR" "sisyphus" \
  "agents diet + все mcps + disabled_hooks = 56 (полный HookNameSchema)"

print_guide "sisyphus" "$VAR" "agents diet + все mcps + disabled_hooks = 56 (полный HookNameSchema)"