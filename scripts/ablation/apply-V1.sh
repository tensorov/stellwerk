#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V1.sh — чистый OpenCode без OMO, только MCP polyforge.
# Повтор старого B (A + polyforge MCP). Проектные конфиги скрываются под
# *.ablation_bak_V1 (наследует суффикс, если уже переименованы V0).
# Глобальный конфиг из .ablation/xdg/V1/opencode/opencode.json (XDG_CONFIG_HOME).

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V1
XDG_NAME="V1"
XDG_CONFIG_FILE="$XDIR_ROOT/$XDG_NAME/opencode/opencode.json"

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

"$ABL_SCRIPT_DIR/gen-configs.sh"
ensure_renamed "$VAR"
mkdir -p "$XDIR_ROOT/$XDG_NAME/opencode"
cp -- "$VARIANTS_DIR/xdg-polyforge.json" "$XDG_CONFIG_FILE"
log "XDG-конфиг: $XDG_CONFIG_FILE"

emit_current_run_env "$VAR" "build" "clean runtime + polyforge (repeat B_qwen=9415)" "$XDIR_ROOT/$XDG_NAME"

cat <<EOF

== NEXT (Пользователь; ассистент opencode не запускается) ==
XDG_CONFIG_HOME="$REPO_ROOT/.ablation/xdg/$XDG_NAME" opencode ~/gits/stellwerk
  (глобальный конфиг = .ablation/xdg/$XDG_NAME/opencode/opencode.json: только polyforge MCP;
   проектные конфиги скрыты -> OMO не загружается)
  Стрелка/Tab -> agent: build
  Проверка: модель = big-pickle.
  Отправь ровно один символ: .
Скопируй число input-токенов и запиши:
./scripts/ablation/record.sh $VAR <tokens> big-pickle "clean runtime + polyforge (повтор B_qwen=9415)"
EOF