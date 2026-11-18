---
id: l01
milestone: V1
status: GREEN
depends_on: [00]
files_allowed: [.sisyphus/plans/N1/packet-04.md, .sisyphus/plans/N1/packet-05.md, .sisyphus/plans/N1/packet-06.md, .sisyphus/plans/N1/packet-08.md, .sisyphus/plans/N1/packet-09.md, .sisyphus/plans/N1/packet-10.md, .sisyphus/plans/N2/packet-00b.md, .sisyphus/plans/N2/packet-00c.md, .sisyphus/plans/N2/packet-00d.md, .sisyphus/plans/N2/packet-00e.md, .sisyphus/plans/N2/packet-00f.md, .sisyphus/plans/N2/packet-00g1.md, .sisyphus/plans/N2/packet-00g3.md, .sisyphus/plans/N2/packet-00h2a.md, .sisyphus/plans/N2/packet-00h2b.md, .sisyphus/plans/N2/packet-00h3a.md, .sisyphus/plans/N2/packet-00h3b.md, .sisyphus/plans/N2/packet-00h3c.md, .sisyphus/plans/N2/packet-00h3d.md, .sisyphus/plans/N2/packet-00h3e.md, .sisyphus/plans/N2/packet-00i1.md, .sisyphus/plans/N2/packet-00j1.md, .sisyphus/plans/N2/packet-00j2.md, .sisyphus/plans/N2/packet-00j3.md, .sisyphus/plans/N2/packet-00j3a.md, .sisyphus/plans/N2/packet-00j4.md, .sisyphus/plans/N2/packet-00j5.md, .sisyphus/plans/N2/packet-00j6.md, .sisyphus/plans/N2/packet-00j7.md, .sisyphus/plans/N2/packet-00j7a.md, .sisyphus/plans/TUI/packet-01.md, .sisyphus/plans/TUI/packet-02.md, .sisyphus/plans/TUI/packet-03.md, crates/stellwerk-cli/tests/plan_lint.rs]
limit_waiver: 34 файла — правка только плана (0 src / 0 test строк по C2), список раскрыт в todo 6
acceptance: cargo test -p stellwerk-cli --test plan_lint
---

# l01 — Закрыть долг plan-lint на каноне

## Цель
Линт плана печатает exit 0 на всём каноне без `--gating`, потому что зелёный линт по подмножеству пакетов не отличим от линта, который ничего не проверил.

## Обоснование (измерено)
- `scripts/plan-lint.py:465` — `WAIVER_RE` принимает строку `overlap_waiver:` как признание пересечения, и без неё пара остаётся нарушением.
- `scripts/plan-lint.py:523` — `check_cross_milestone_overlap` сообщает C9 только для неупорядоченных пар разных milestone.
- `scripts/plan-lint.py:381` — `related` считает связь по `depends_on` транзитивно в обе стороны, поэтому порядок объявляется одним краем пары.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/plan_lint.rs`, тест `plan_lint_is_green`: запускает `python3 scripts/plan-lint.py`, требует exit 0 и отсутствия подстроки `НАРУШЕНИЙ`; до 33 правок линт печатает 77 нарушений и возвращает 1, поэтому тест падает на коде возврата и на найденной подстроке.

## Приёмка
cargo test -p stellwerk-cli --test plan_lint
Ожидается PASS.

## Запрещено
- нельзя менять `files_allowed`, `goal`, `acceptance` или тело существующих пакетов.
- запрещено использовать `--gating`, чтобы спрятать нарушения.
- нельзя править `.omo/plans/` как источник или трогать `.rs` вне нового теста.
