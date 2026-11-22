"""Repository-local OMO configuration guard.

Semantic check of `.omo/omo.jsonc` and `opencode.json` against the
repository's model-routing contract:

* 183 `[opencode].agents` + 8 `categories`, each with the ordered chain
  deepseek-v4.1-flash-free -> space-bunny-free -> big-pickle;
* `momus` and `metis` are reviewers: primary deepseek-v4.1-flash-free with
  space-bunny-free -> big-pickle fallback (same chain as every other agent);
* big-pickle is never a primary;
* every primary model has a documented context floor >= 200_000
  (observed floors: space-bunny 1_048_576, deepseek 200_000,
  nemotron 1_000_000);
* `opencode.json` pins the OMO plugin to `oh-my-openagent@4.19.4`
  (no floating `@latest`);
* public-only: every referenced model is in the observed public catalog
  and big-pickle occurs only in `fallback_models`, never as `model`.

Every agent and category is routed to deepseek-v4.1-flash-free as its
primary model; space-bunny-free is the first fallback.
"""

from __future__ import annotations

import json
from collections.abc import Iterator
from pathlib import Path
from typing import TypedDict

REPO_ROOT = Path(__file__).resolve().parents[1]
OMO_PATH = REPO_ROOT / ".omo" / "omo.jsonc"
OPENCODE_PATH = REPO_ROOT / "opencode.json"

EXPECTED_AGENT_COUNT = 183
EXPECTED_CATEGORY_COUNT = 8

GENERAL_PRIMARY = "opencode/deepseek-v4.1-flash-free"
GENERAL_FALLBACKS = ["opencode/space-bunny-free", "opencode/big-pickle"]
REVIEWER_PRIMARY = "opencode/deepseek-v4.1-flash-free"
REVIEWER_FALLBACKS = ["opencode/space-bunny-free", "opencode/big-pickle"]
REVIEWER_AGENTS = frozenset({"momus", "metis"})

PLUGIN_PIN = "oh-my-openagent@4.19.4"
PLUGIN_FLOATING = "oh-my-openagent@latest"

# Observed context-window floors (tokens) for public models that may serve
# as primaries. Source: opencode model catalog (opencode provider).
# big-pickle is deliberately absent: it is fallback-only and must never be
# a primary, so its floor is never consulted.
MODEL_FLOORS = {
    "opencode/space-bunny-free": 1_048_576,
    "opencode/deepseek-v4.1-flash-free": 200_000,
    "opencode/nemotron-3-ultra-free": 1_000_000,
}
MIN_PRIMARY_FLOOR = 200_000  # deepseek's floor: no primary below this.
PUBLIC_CATALOG = frozenset(MODEL_FLOORS) | {"opencode/big-pickle"}


class AgentEntry(TypedDict):
    model: str
    fallback_models: list[str]


class OpencodeSection(TypedDict):
    agents: dict[str, AgentEntry]


OmoConfig = TypedDict(
    "OmoConfig",
    {
        "[opencode]": OpencodeSection,
        "categories": dict[str, AgentEntry],
    },
)


def strip_jsonc_comments(text: str) -> str:
    """Remove // and /* */ comments without touching string literals."""
    out: list[str] = []
    i, n = 0, len(text)
    in_string = False
    while i < n:
        c = text[i]
        if in_string:
            out.append(c)
            if c == "\\":
                i += 1
                if i < n:
                    out.append(text[i])
            elif c == '"':
                in_string = False
            i += 1
            continue
        if c == '"':
            in_string = True
            out.append(c)
            i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            i += 2
            while i + 1 < n and not (text[i] == "*" and text[i + 1] == "/"):
                i += 1
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def load_omo_config() -> OmoConfig:
    text = OMO_PATH.read_text(encoding="utf-8")
    return json.loads(strip_jsonc_comments(text))


def load_opencode_config() -> dict[str, object]:
    return json.loads(OPENCODE_PATH.read_text(encoding="utf-8"))


def _assert_eq(key: str, expected: object, actual: object) -> None:
    assert actual == expected, f"{key}: expected={expected!r}, actual={actual!r}"


def _all_entries(cfg: OmoConfig) -> Iterator[tuple[str, str, AgentEntry]]:
    for name, entry in cfg["[opencode]"]["agents"].items():
        yield "[opencode].agents", name, entry
    for name, entry in cfg["categories"].items():
        yield "categories", name, entry


def test_omo_jsonc_parses_as_jsonc() -> None:
    cfg = load_omo_config()
    assert isinstance(cfg, dict)


def test_opencode_json_parses() -> None:
    cfg = load_opencode_config()
    assert isinstance(cfg, dict)


def test_agent_and_category_counts() -> None:
    cfg = load_omo_config()
    _assert_eq(
        "[opencode].agents count",
        EXPECTED_AGENT_COUNT,
        len(cfg["[opencode]"]["agents"]),
    )
    _assert_eq("categories count", EXPECTED_CATEGORY_COUNT, len(cfg["categories"]))


def test_general_model_chain() -> None:
    cfg = load_omo_config()
    for section, name, entry in _all_entries(cfg):
        if name in REVIEWER_AGENTS:
            continue
        _assert_eq(f"{section}[{name}].model", GENERAL_PRIMARY, entry.get("model"))
        _assert_eq(
            f"{section}[{name}].fallback_models",
            GENERAL_FALLBACKS,
            entry.get("fallback_models"),
        )


def test_reviewer_agents_route_to_deepseek() -> None:
    cfg = load_omo_config()
    agents = cfg["[opencode]"]["agents"]
    for name in sorted(REVIEWER_AGENTS):
        entry = agents.get(name)
        assert entry is not None, f"[opencode].agents[{name}]: missing entry"
        _assert_eq(
            f"[opencode].agents[{name}].model",
            REVIEWER_PRIMARY,
            entry.get("model"),
        )
        _assert_eq(
            f"[opencode].agents[{name}].fallback_models",
            REVIEWER_FALLBACKS,
            entry.get("fallback_models"),
        )


def test_primary_models_have_documented_floor() -> None:
    cfg = load_omo_config()
    for section, name, entry in _all_entries(cfg):
        model = entry.get("model")
        assert model is not None, f"{section}[{name}].model: missing"
        floor = MODEL_FLOORS.get(model)
        assert floor is not None, (
            f"{section}[{name}].model: unknown model {model!r}; "
            f"known floors={MODEL_FLOORS}"
        )
        assert floor >= MIN_PRIMARY_FLOOR, (
            f"{section}[{name}].model: floor {floor} < {MIN_PRIMARY_FLOOR}"
        )


def test_public_only_models() -> None:
    cfg = load_omo_config()
    for section, name, entry in _all_entries(cfg):
        model = entry.get("model")
        assert model is not None, f"{section}[{name}].model: missing"
        assert model in PUBLIC_CATALOG, (
            f"{section}[{name}].model: {model!r} not in public catalog "
            f"{sorted(PUBLIC_CATALOG)}"
        )
        assert model != "opencode/big-pickle", (
            f"{section}[{name}].model: big-pickle may occur only in "
            f"fallback_models, never as model"
        )
        for fb in entry.get("fallback_models") or []:
            assert fb in PUBLIC_CATALOG, (
                f"{section}[{name}].fallback_models: {fb!r} not in public "
                f"catalog {sorted(PUBLIC_CATALOG)}"
            )


def test_plugin_pin() -> None:
    cfg = load_opencode_config()
    plugins = cfg.get("plugin", [])
    assert isinstance(plugins, list), f"plugin: expected a list, actual={plugins!r}"
    assert PLUGIN_PIN in plugins, f"plugin: expected pin {PLUGIN_PIN!r} in {plugins!r}"
    assert PLUGIN_FLOATING not in plugins, (
        f"plugin: floating tag {PLUGIN_FLOATING!r} must be replaced by "
        f"{PLUGIN_PIN!r}; actual={plugins!r}"
    )