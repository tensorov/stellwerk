set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

# ─── GATE: строгие проверки, падают при любой ошибке ────────────────────────
# ПРАВИЛО: крейт переезжает в gate в момент закрытия его milestone.
#
# Пока milestone активен, красные тесты в его крейте — это ожидаемая
# RED-спецификация, и гейт обязан её терпеть. Как только milestone закрыт, тот же
# красный тест становится регрессией, и гейт обязан падать. Список крейтов в
# gate и в status — не вкусовое, а выводится из этого правила.
#
# Закрытые milestone'ы: M0 (stellwerk-registry), M1 (stellwerk-core, stellwerk-store).
test-registry:
    cargo test -p stellwerk-registry --locked --no-fail-fast

test-core:
    cargo test -p stellwerk-core --locked --no-fail-fast

test-store:
    cargo test -p stellwerk-store --locked --no-fail-fast

check-gate: fmt lint test-registry test-core test-store

# ─── STATUS: информативные проверки, НЕ падают ─────────────────────────────
# Только крейты АКТИВНЫХ milestone'ов: M2 (stellwerk-task), M3 (stellwerk-gateway).
# Печатают счётчики GREEN/RED, чтобы «сколько спецификации осталось» было видно
# в логе без чтения отчёта. Перечислять все крейты workspace нельзя: молчащий
# отчёт о крейте, которого здесь нет, занижает остаток работ (F-06).
# --no-fail-fast обязателен: без него cargo останавливается после первого
# Счётчики GREEN/RED по крейтам активных milestone. Не падает.
# Приёмка C7: документированная таблица risk_class обязана совпадать с
# фактической матрицей в contract.rs.
#
# Пункт 3 F-2026-09-27-07: правка кода обязана обновлять таблицы тем же
# коммитом. Раньше это было написано в FINDINGS, но не проверялось ничем.
# Теперь расхождение роняет гейт, поэтому забыть обновить таблицу нельзя.
#
# В гейт идёт только C7. C1-C6 остаются информационными: у M0 16 исторических
# нарушений, и если бы они влияли на код возврата, гейт был бы всегда красным,
# а сверка таблицы — никогда не выполнялась бы.
check-risk-tables:
    cargo test --locked -p stellwerk-core --test risk_level_policy
    cargo test --locked -p stellwerk-gateway --test permission --test gate --test gate_ask

test-status:
    @for c in stellwerk-task stellwerk-gateway; do \
        out=$(cargo test -p "$c" --no-fail-fast 2>&1 || true); \
        failed=$(printf '%s' "$out" | grep -E "^test .* \.\.\. FAILED" | awk '{print $2}' | sort -u | wc -l || true); \
        passed=$(printf '%s' "$out" | grep -E "^test .* \.\.\. ok" | awk '{print $2}' | sort -u | wc -l || true); \
        failed=${failed:-0}; passed=${passed:-0}; \
        total=$(( passed + failed )); \
        echo "$c: $passed/$total ($failed RED, expected — активный milestone)"; \
    done

# Информационный RED-отчёт по активным milestone. Не входит в строгий gate
# и не роняет его кодом возврата: test-status считает счётчики, а не вердикт.
status: test-status
    @echo "test-status: informational RED report — does not fail the gate"

# Обратная совместимость: прежний псевдоним теперь ведёт в informational-сводку.
check-status: status

# Строгие проверки публикации: история, наборы скриптов и публичное дерево.
history:
    bash scripts/scan-history.sh

scripts:
    for f in scripts/tests/*.sh; do bash "$f"; done

public-tree:
    cargo test -p stellwerk-registry --test repo_hygiene

# Основная точка входа. Только строгий gate; RED-отчёт вызывается отдельно.
check: check-gate check-risk-tables history scripts public-tree

# Полный прогон всего, включая падающие наборы. Падает — для ручной отладки.
test-all:
    cargo test --workspace --no-fail-fast

# приёмка плана Prometheus до вызова Momus/Metis
plan-lint:
    ./scripts/plan-lint.sh

# приёмка dev-станции (уровень 0)
doctor:
    @sh scripts/doctor.sh
