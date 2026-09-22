#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V9.sh — ЧИСТЫЙ runtime (без OMO) + stellwerk-micro + polyforge.
# Проектные конфиги скрываются под *.ablation_bak_V9 (наследует суффикс от V0..V1).
# Глобальный конфиг из .ablation/xdg/V9/opencode/opencode.json:
#   xdg-polyforge + default_agent=stellwerk-micro;
#   агент .ablation/xdg/V9/opencode/agents/stellwerk-micro.md (глобальный, XDG).
# Цель: чистым runtime мерим micro — это нижняя граница.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V9
XDG_NAME="V9"
XDG_ROOT="$XDIR_ROOT/$XDG_NAME"
XDG_CONFIG_FILE="$XDG_ROOT/opencode/opencode.json"

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

"$ABL_SCRIPT_DIR/gen-configs.sh"
ensure_renamed "$VAR"
mkdir -p "$XDG_ROOT/opencode/agents"

jq '.default_agent = "stellwerk-micro"' "$VARIANTS_DIR/xdg-polyforge.json" \
  > "$XDG_CONFIG_FILE.tmp"
chmod 600 -- "$XDG_CONFIG_FILE.tmp"
mv -- "$XDG_CONFIG_FILE.tmp" "$XDG_CONFIG_FILE"
log "XDG-конфиг: $XDG_CONFIG_FILE (+ default_agent=stellwerk-micro)"

cp -- "$ABL_SCRIPT_DIR/variants/stellwerk-micro.md" "$XDG_ROOT/opencode/agents/stellwerk-micro.md"
log "агент: $XDG_ROOT/opencode/agents/stellwerk-micro.md"

emit_current_run_env "$VAR" "stellwerk-micro" "clean runtime + polyforge + stellwerk-micro (без OMO)" "$XDG_ROOT"

cat <<EOF

== NEXT (Пользователь; ассистент opencode не запускается) ==
XDG_CONFIG_HOME="$REPO_ROOT/.ablation/xdg/$XDG_NAME" opencode ~/gits/stellwerk
  (глобальный конфиг = .ablation/xdg/$XDG_NAME/opencode/opencode.json: только polyforge MCP +
   агент stellwerk-micro; проектные конфиги скрыты -> OMO не загружается)
  Проверка: в строке состояния агент = stellwerk-micro (default_agent), модель = big-pickle.
  Если агент не default: Стрелка/Tab -> agent: stellwerk-micro.
  Отправь ровно один символ: .
Скопируй число input-токенов и запиши:
./scripts/ablation/record.sh $VAR <tokens> big-pickle "clean runtime + polyforge + stellwerk-micro (без OMO)"
EOF