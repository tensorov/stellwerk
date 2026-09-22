#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V2.sh — ordinary OMO stack (как используется сейчас):
# OMO plugin + текущий OpenCode MCP поверх (polyforge on, grep_app off, lsp off).
# Ближе к старому D/E, НЕ к C. ~/.omo/omo.jsonc НЕ меняется (полный OMO).
# Проектные opencode.json/.opencode возвращаются на место (если скрыты от V0/V1).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V2

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
log "~/.omo/omo.jsonc НЕ трогаем — OMO как сейчас (носитель: обычный вариант)."

emit_current_run_env "$VAR" "sisyphus" \
  "ordinary OMO stack = OMO + текущий MCP поверх (≈ старый D/E, не C); omo.jsonc не менялся"

print_guide "sisyphus (OMS default)" "$VAR" \
  "ordinary OMO stack = OMO + текущий MCP поверх (≈ старый D/E, не C); omo.jsonc не менялся"