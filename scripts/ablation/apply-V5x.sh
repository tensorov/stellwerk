#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V5x.sh — V5h + disabled_commands ПОЛНЫЙ enum (goal, refactor, start-work,
# stop-continuation, remove-ai-slops, hyperplan).
# disabled_skills/disabled_tools — открытые массивы (без enum) -> NOT_APPLICABLE.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V5x

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v5x.jsonc"

emit_current_run_env "$VAR" "sisyphus" \
  "полный diet: agents + mcps + hooks(56) + commands(6); skills/tools NOT_APPLICABLE"

print_guide "sisyphus" "$VAR" "полный diet: agents + mcps + hooks(56) + commands(6); skills/tools NOT_APPLICABLE"