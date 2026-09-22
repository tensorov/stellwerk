#!/usr/bin/env bash
# -*- mode: sh; sh-shell: bash -*-
#
# gen-configs.sh — генерирует whole-file варианты конфигов из ЖИВОГО
# ~/.config/opencode/opencode.json (только чтение источника). Результаты:
#   .ablation/variants/global_opencode_omo.json  — OMO-варианты (P3):
#         mcp.polyforge enabled:true, mcp.grep_app enabled:false, mcp.lsp enabled:false;
#         provider/model/small_model/disabled_providers и пр. — как в живом конфиге.
#   .ablation/variants/xdg-clean.json            — XDG V0: без mcp, lsp, plugin (P2).
#   .ablation/variants/xdg-polyforge.json        — XDG V1/V9: mcp.polyforge enabled:true (P2).
#
# Ключи API в .ablation/variants/ (gitignored). В командной строке/логах содержимое
# никогда не печатается — только путь и sha256.

set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "$SCRIPT_DIR/_lib.sh"

command -v jq >/dev/null 2>&1 || die "jq не найден (нужен для JSON-безопасной трансформации; уже установлен в системе)"

src="$ABL_HOME/.config/opencode/opencode.json"
[ -f "$src" ] || die "нет живого глобального конфига: $src"

mkdir -p -- "$VARIANTS_DIR"

tmp="$VARIANTS_DIR/.tmp.$$"

# OMO-вариант глобального конфига (P3): единое MCP-состояние.
jq '(.mcp // {} | .polyforge.enabled = true | .grep_app.enabled = false | .lsp.enabled = false) as $m | .mcp = $m' \
  "$src" > "$tmp"
chmod 600 -- "$tmp"
mv -- "$tmp" "$VARIANTS_DIR/global_opencode_omo.json"

# XDG V0: без mcp, lsp, plugin (P2). provider/model/small_model/disabled_providers остаются.
jq 'del(.mcp, .lsp, .plugin)' "$src" > "$tmp"
chmod 600 -- "$tmp"
mv -- "$tmp" "$VARIANTS_DIR/xdg-clean.json"

# XDG V1/V9: только polyforge MCP, autoload (P2).
jq 'del(.lsp, .plugin) | .mcp = { polyforge: { type: "local", command: ["polyforge-mcp"], enabled: true } }' \
  "$src" > "$tmp"
chmod 600 -- "$tmp"
mv -- "$tmp" "$VARIANTS_DIR/xdg-polyforge.json"

log "сгенерированы (без вывода содержимого):"
for f in global_opencode_omo.json xdg-clean.json xdg-polyforge.json; do
  log "  $VARIANTS_DIR/$f  sha256=$(sha256sum "$VARIANTS_DIR/$f" | awk '{print $1}')"
done