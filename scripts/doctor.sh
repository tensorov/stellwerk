#!/bin/sh
# Fail-closed prerequisite check for the dev station.
# Collects every status line before exiting non-zero, so a single run shows
# the full picture (including UNCONFIGURED) instead of stopping at the first gap.
set -u

failed=0

check_tool() {
    tool_name=$1
    tool_hint=$2
    if command -v "$tool_name" >/dev/null 2>&1; then
        printf '%s: OK (found on PATH)\n' "$tool_name"
    else
        printf '%s: MISSING (%s)\n' "$tool_name" "$tool_hint"
        failed=1
    fi
}

check_tool cargo 'install the Rust toolchain (cargo)'
check_tool rustc 'install the Rust toolchain (rustc)'
check_tool opencode 'install opencode CLI'
check_tool polyforge-mcp 'install the polyforge MCP server binary'

# gh is optional for local development; publication requires it (task 22).
if command -v gh >/dev/null 2>&1; then
    printf 'gh: OK (found on PATH)\n'
else
    printf 'gh: MISSING (optional for local development; required for publication)\n'
fi

# Pi: PI_BIN first, then $HOME/.pi/agent/bin/pi — same rule as the CLI.
if [ -n "${PI_BIN-}" ]; then
    pi_path=$PI_BIN
elif [ -n "${HOME-}" ]; then
    pi_path=$HOME/.pi/agent/bin/pi
else
    pi_path=
fi
if [ -n "$pi_path" ] && [ -f "$pi_path" ]; then
    printf 'pi: OK (%s)\n' "$pi_path"
else
    printf 'pi: MISSING (set PI_BIN or install Pi under $HOME/.pi/agent/bin/pi)\n'
    failed=1
fi

# Pi extension: unset is a distinct UNCONFIGURED state, not a missing file.
if [ -z "${STELLWERK_PI_EXTENSION+x}" ]; then
    printf 'STELLWERK_PI_EXTENSION: UNCONFIGURED (set it to the Pi extension file path)\n'
    failed=1
elif [ -f "$STELLWERK_PI_EXTENSION" ]; then
    printf 'STELLWERK_PI_EXTENSION: OK (%s)\n' "$STELLWERK_PI_EXTENSION"
else
    printf 'STELLWERK_PI_EXTENSION: MISSING (file not found: %s)\n' "$STELLWERK_PI_EXTENSION"
    failed=1
fi

# Token: readable and non-empty. File mode is a separate finding, not checked here.
if [ -n "${XDG_CONFIG_HOME-}" ]; then
    token_root=$XDG_CONFIG_HOME
elif [ -n "${HOME-}" ]; then
    token_root=$HOME/.config
else
    token_root=
fi
if [ -n "$token_root" ]; then
    token_path=$token_root/stellwerk/neuraldeep.token
else
    token_path=
fi
if [ -n "$token_path" ] && [ -f "$token_path" ] && [ -r "$token_path" ] && [ -s "$token_path" ]; then
    printf 'token: OK (%s)\n' "$token_path"
else
    printf 'token: MISSING (need a readable non-empty $XDG_CONFIG_HOME/stellwerk/neuraldeep.token)\n'
    failed=1
fi

check_tool bwrap 'install bubblewrap (bwrap) for the task sandbox'

# Preserved from the original inline doctor recipe.
if [ -d .sisyphus/plans ]; then
    printf '.sisyphus/plans: ok\n'
else
    printf '.sisyphus/plans: MISSING (expected .sisyphus/plans in the repository root)\n'
    failed=1
fi
if grep -q '"subagent_depth": 1' opencode.json 2>/dev/null; then
    printf 'subagent_depth=1: ok\n'
else
    printf 'subagent_depth=1: MISSING (expected "subagent_depth": 1 in opencode.json)\n'
    failed=1
fi

exit "$failed"
