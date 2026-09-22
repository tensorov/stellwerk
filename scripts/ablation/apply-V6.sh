#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V6.sh — ВОПРОС: ест ли OMO tax даже для НЕ-OMO агента?
#   OMO ВКЛЮЧЁН (диета = V5x), активный агент = кастомный OpenCode primary
#   agent `stellwerk-micro` (.opencode/agents/stellwerk-micro.md),
#   default_agent = "stellwerk-micro" (в проектный opencode.json).
#   sisyphus prompt НЕ переопределяется (это делает V7).
# Настоящий проектный opencode.json сохраняется в .ablation/variants/project_opencode_real.json
# (apply-V7 вернёт его на место).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V6
AGENT_MD="$ABL_SCRIPT_DIR/variants/stellwerk-micro.md"
PROJ_CFG="$REPO_ROOT/opencode.json"

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

ensure_live_omo
install_global_omo
install_omo_config "$ABL_SCRIPT_DIR/variants/omo_v5x.jsonc"

# 1) настоящий проектный конфиг — на бэкап для V7
cp -- "$PROJ_CFG" "$VARIANTS_DIR/project_opencode_real.json"
chmod 600 -- "$VARIANTS_DIR/project_opencode_real.json"

# 2) V6-вариант проектного opencode.json: + default_agent = stellwerk-micro
command -v jq >/dev/null 2>&1 || die "jq не найден"
jq '.default_agent = "stellwerk-micro"' "$PROJ_CFG" > "$VARIANTS_DIR/project_opencode_v6.json.tmp"
chmod 600 -- "$VARIANTS_DIR/project_opencode_v6.json.tmp"
mv -- "$VARIANTS_DIR/project_opencode_v6.json.tmp" "$VARIANTS_DIR/project_opencode_v6.json"

# 3) применить V6-проектный конфиг
cp -- "$VARIANTS_DIR/project_opencode_v6.json" "$PROJ_CFG"
log "проектный opencode.json -> V6 (+ default_agent=stellwerk-micro)"

# 4) кастомный агент
mkdir -p -- "$REPO_ROOT/.opencode/agents"
cp -- "$AGENT_MD" "$REPO_ROOT/.opencode/agents/stellwerk-micro.md"
log "агент: $REPO_ROOT/.opencode/agents/stellwerk-micro.md"

emit_current_run_env "$VAR" "stellwerk-micro" \
  "custom primary agent stellwerk-micro внутри OMO (диета V5x); sisyphus prompt не менялся"

cat <<EOF

== NEXT (Пользователь; ассистент opencode не запускается) ==
  opencode ~/gits/stellwerk
  Проверка 1: в строке состояния агент = stellwerk-micro (default_agent), модель = big-pickle.
  Проверка 2: если UI вместо stellwerk-micro показывает ТОЛЬКО sisyphus -> сообщи, замерь всё равно:
      ./scripts/ablation/record.sh $VAR <tokens> big-pickle "NOT_APPLICABLE OMO overrides custom default_agent"
  Проверка 3: если Tab -> агент stellwerk-micro вообще НЕ виден:
      ./scripts/ablation/record.sh $VAR <tokens> big-pickle "NOT_APPLICABLE OMO hides custom agent"
  Если виден и активен:
      отправь ровно один символ: .
      ./scripts/ablation/record.sh $VAR <tokens> big-pickle "custom primary agent stellwerk-micro внутри OMO (диета V5x)"
EOF