# RED baseline — stellwerk-store

Снято интегратором в сессии #2, Фаза 6, до раздачи пакетов 08–12.
`--no-fail-fast` обязателен (см. core baseline).

```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 3 tests
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
error: test failed, to rerun pass `-p stellwerk-store --test concurrent`
running 4 tests
test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
error: test failed, to rerun pass `-p stellwerk-store --test event_append`
running 7 tests
test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
error: test failed, to rerun pass `-p stellwerk-store --test module_store`
running 7 tests
test result: FAILED. 1 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
error: test failed, to rerun pass `-p stellwerk-store --test task_crud`
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Причина каждого падения (перечень заглушек)

       37 not yet implemented: RED: SqliteStore::append
        6 not yet implemented: RED: SqliteStore::upsert_lock_entry
        4 not yet implemented: RED: SqliteStore::create
        2 not yet implemented: RED: SqliteStore::get
        1 not yet implemented: RED: SqliteStore::update_status

## Сводка

GREEN=1  RED=20  TOTAL=21
