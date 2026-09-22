#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# apply-V0.sh — чистый OpenCode без OMO и без MCP (повтор старого A).
# Проектные opencode.json и .opencode прячутся под *.ablation_bak_V0,
# глобальный конфиг берётся из .ablation/xdg/V0/opencode/opencode.json (XDG_CONFIG_HOME).
# Модель: big-pickle (взято из живого глобального конфига), счётчик провайдера.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

VAR=V0
XDG_NAME="V0"
XDG_CONFIG_FILE="$XDIR_ROOT/$XDG_NAME/opencode/opencode.json"

snap="$("$SCRIPT_DIR/snapshot.sh" "pre-$VAR")"
log "снапшот до $VAR: $snap"

"$ABL_SCRIPT_DIR/gen-configs.sh"
ensure_renamed "$VAR"
mkdir -p "$XDIR_ROOT/$XDG_NAME/opencode"
cp -- "$VARIANTS_DIR/xdg-clean.json" "$XDG_CONFIG_FILE"
log "XDG-конфиг: $XDG_CONFIG_FILE"

emit_current_run_env "$VAR" "build" "clean runtime, без OMO и без MCP (repeat A_qwen=8602)" "$XDIR_ROOT/$XDG_NAME"

cat <<EOF

== NEXT (Пользователь; ассистент opencode не запускается) ==
XDG_CONFIG_HOME="$REPO_ROOT/.ablation/xdg/$XDG_NAME" opencode ~/gits/stellwerk
  (глобальный конфиг = .ablation/xdg/$XDG_NAME/opencode/opencode.json: без mcp/lsp/plugin;
   проектные конфиги скрыты -> OMO и MCP не загружаются)
  Стрелка/Tab -> agent: build
  Проверка: модель = big-pickle.
  Отправь ровно один символ: .
Скопируй число input-токенов и запиши:
./scripts/ablation/record.sh $VAR <tokens> big-pickle "clean runtime, без OMO и без MCP (повтор A_qwen=8602)"
EOF