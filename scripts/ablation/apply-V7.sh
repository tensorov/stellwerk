#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V7.sh — ВОПРОС: можно ли сделать тонкий штатный Sisyphus внутри OMO?
#   OMO ВКЛЮЧЁН (диета = V5x) + agents.sisyphus.prompt = микро-промпт (omo_v7.jsonc).
#   Активный агент: sisyphus (Tab -> sisyphus; default_agent из V6 не требуется).
#   Кастомный агент stellwerk-micro из V6 и default_agent убираются,
#   настоящий проектный opencode.json возвращается с бэкапа проекта V6.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V7
AGENT_MD="$REPO_ROOT/.opencode/agents/stellwerk-micro.md"
PROJ_CFG="$REPO_ROOT/opencode.json"
REAL_CFG="$VARIANTS_DIR/project_opencode_real.json"

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo

# вернуть настоящий проектный конфиг (убрать default_agent из V6)
if [ -f "$REAL_CFG" ]; then
  cp -- "$REAL_CFG" "$PROJ_CFG"
  log "проектный opencode.json -> нативный (default_agent=stellwerk-micro убран)"
else
  die "нет бэкапа настоящего проектного конфига: $REAL_CFG (apply-V6 не запускался?)"
fi

# убрать кастомный агент V6 (если остался)
[ -f "$AGENT_MD" ] && { rm -f -- "$AGENT_MD"; log "убран агент V6: $AGENT_MD"; }

install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v7.jsonc"

emit_current_run_env "$VAR" "sisyphus" \
  "диета V5x + agents.sisyphus.prompt = микро; sisyphus активен"

print_guide "sisyphus (тонкий через override prompt)" "$VAR" \
  "диета V5x + agents.sisyphus.prompt = микро; sisyphus активен; Tab -> sisyphus"