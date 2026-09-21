//! Базовый smoke: workspace собирается и тесты запускаются. Не заменяет TDD-тесты packets.
#[test]
fn workspace_compiles_and_tests_run() {
    assert_eq!(2 + 2, 4);
}
