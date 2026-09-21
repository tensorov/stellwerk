set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

check: fmt lint test

# приёмка плана Prometheus до вызова Momus/Metis
plan-lint:
    ./scripts/plan-lint.sh

# приёмка dev-станции (уровень 0)
doctor:
    @echo "opencode:  $(opencode --version 2>/dev/null || echo MISSING)"
    @echo "polyforge: $(command -v polyforge-mcp || echo MISSING)"
    @echo "gh:        $(gh --version 2>/dev/null | head -1 || echo MISSING)"
    @echo "cargo:     $(cargo --version)"
    @test -d .sisyphus/plans && echo ".sisyphus/plans: ok"
    @grep -q '"subagent_depth": 1' opencode.json && echo "subagent_depth=1: ok"
