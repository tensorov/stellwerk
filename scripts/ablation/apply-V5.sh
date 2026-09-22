#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V5.sh — V4 + disabled_mcps ПОЛНЫЙ enum (websearch, context7, grep_app, lsp, codegraph).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V5

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v5.jsonc"

emit_current_run_env "$VAR" "sisyphus" \
  "agents diet + disabled_mcps = все 5 (grep_app, lsp добавлены к базовым 3)"

print_guide "sisyphus" "$VAR" "agents diet + disabled_mcps = все 5 (grep_app, lsp добавлены к базовым 3)"