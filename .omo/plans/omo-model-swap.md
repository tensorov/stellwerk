---
status: done
created: 2026-11-09
scope: tooling-config
executor: /start-work
---

# OMO model swap: `mimo-v2.6-flash-free` → `deepseek-v4.1-flash-free`

## Goal
Replace `opencode/mimo-v2.6-flash-free` with `opencode/deepseek-v4.1-flash-free` in the repo-tracked OMO config, and keep the repository guard test green.

## Verified facts (2026-11-09)
- Tracked config: `.omo/omo.jsonc` contains **191** occurrences of `opencode/mimo-v2.6-flash-free` (183 agents + 8 categories).
- Global `~/.omo/omo.jsonc` contains **no** mimo (uses `opencode/big-pickle` / `opencode/nemotron-3-ultra-free`). Do not touch it.
- Catalog (`opencode models`, `~/.cache/opencode/models.json`): `opencode/deepseek-v4.1-flash-free` exists with `context=200000`, `output=32000` — satisfies the guard's ≥200k floor.
- Guard test baseline: `python3 -m pytest -p no:aiohttp tests/test_omo_config.py -q` → **8 passed**.
- `pytest_aiohttp` is broken in this env (`ModuleNotFoundError: pytest_asyncio`), so `-p no:aiohttp` is required.
- `opencode.json` already pins `oh-my-openagent@4.19.4`; leave it unchanged.

## TODOs
- [x] 1. `.omo/omo.jsonc`: replace every `opencode/mimo-v2.6-flash-free` with `opencode/deepseek-v4.1-flash-free` (191 occurrences). Change nothing else.
- [x] 2. `tests/test_omo_config.py`: update docstring (`mimo` → `deepseek`), `GENERAL_FALLBACKS` → `["opencode/deepseek-v4.1-flash-free", "opencode/big-pickle"]`, `REVIEWER_PRIMARY` → `"opencode/deepseek-v4.1-flash-free"`, replace the mimo `MODEL_FLOORS` entry with `"opencode/deepseek-v4.1-flash-free": 200_000`, update the `MIN_PRIMARY_FLOOR` comment, and rename `test_reviewer_agents_route_to_mimo` → `test_reviewer_agents_route_to_deepseek`.
- [x] 3. Run the Final Verification Wave and confirm every expected result.

## Final Verification Wave
- [x] F1. `rg -n "mimo" .omo/omo.jsonc tests/test_omo_config.py` → no matches.
- [x] F2. `python3 -m pytest -p no:aiohttp tests/test_omo_config.py -q` → 8 passed.
- [x] F3. `git diff --stat` → only `.omo/omo.jsonc` and `tests/test_omo_config.py` changed.

## Constraints
- Do **not** touch `~/.omo/omo.jsonc`, `opencode.json`, or any other config.
- Do **not** change `GENERAL_PRIMARY`, `REVIEWER_FALLBACKS`, `space-bunny-free`, `nemotron`, or the `big-pickle` fallback-only role.
- No product code, no new dependencies, no ADR (model routing is not a lint-policy/contract-change file).
