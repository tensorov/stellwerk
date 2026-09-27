# RED baseline — stellwerk-core

Снято интегратором в сессии #2, Фаза 6, до раздачи пакетов 02–06.
Цель: доказать, что красное падает по задуманной причине
`not yet implemented: RED: …`, а не по ошибке компиляции или неверному assert.
`--no-fail-fast` обязателен: без него cargo останавливается на первом упавшем
наборе и показывает 1/7 вместо честных 20/47.

```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 7 tests
test result: FAILED. 1 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p stellwerk-core --test data_class_routing`
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 11 tests
test result: FAILED. 4 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p stellwerk-core --test rbac_level`
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 11 tests
test result: FAILED. 1 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p stellwerk-core --test task_lifecycle`
running 8 tests
test result: FAILED. 4 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p stellwerk-core --test workspace_policy`
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Причина каждого падения (перечень заглушек)

        9 not yet implemented: RED: TaskStatus::can_transition_to
        7 not yet implemented: RED: PolicyLevel::permits
        6 not yet implemented: RED: DataClass::permits_model
        4 not yet implemented: RED: NetworkPolicy::allows
        1 not yet implemented: RED: TaskStatus::is_terminal

## Сводка

GREEN=20  RED=27  TOTAL=47
