set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

# ─── GATE: строгие проверки, падают при любой ошибке ────────────────────────
# Ровно они определяют, можно ли мержить. stellwerk-core и stellwerk-store
# сюда НЕ входят намеренно: они содержат RED-спецификацию (todo!()), которая
# записывает будущую работу как падающие тесты. Включение их в gate означало бы,
# что «зелёный» статус CI означает «спецификация удалена», а не «работа сделана».
# Пока RED не превращён в GREEN пакетами M1, фиксируем это разделением рецептов.
test-registry:
    cargo test -p stellwerk-registry --locked --no-fail-fast

check-gate: fmt lint test-registry

# ─── STATUS: информативные проверки, НЕ падают ─────────────────────────────
# Печатают счётчики GREEN/RED, чтобы «сколько спецификации осталось» было
# видно в логе без чтения отчёта. --no-fail-fast обязателен: без него cargo
# останавливается после первого упавшего набора и показывает 1/7 вместо 20/47.
test-status:
    @for c in stellwerk-core stellwerk-store; do \
        counts=$(cargo test -p "$c" --no-fail-fast 2>&1 | grep -E "^test result:" | awk -F"[ ;.]+" '{p += $4; f += $6} END {print p+0, f+0}' || true); \
        set -- $counts; \
        total=$(( $1 + $2 )); \
        echo "$c: $1/$total ($2 RED, expected — architecture spec)"; \
    done

check-status: test-status

# Основная точка входа. Строгий gate + обязательная сводка по спецификации.
check: check-gate test-status

# Полный прогон всего, включая падающие наборы. Падает — для ручной отладки.
test-all:
    cargo test --workspace --no-fail-fast

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
