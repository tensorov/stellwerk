#!/usr/bin/env bash
# Repository-local OMO configuration guard.
#
# Runs the same semantic check as `pytest tests/test_omo_config.py -q`
# from the repo root, isolated and deterministic: fixed cwd, no bytecode
# writes, no plugin autoload (environment plugin sets cannot break it),
# no network, exit code propagated.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

export PYTHONDONTWRITEBYTECODE=1
export PYTEST_DISABLE_PLUGIN_AUTOLOAD=1
exec python3 -m pytest tests/test_omo_config.py -q "$@"