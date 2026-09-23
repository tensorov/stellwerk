

Теперь вижу рабочее окружение. Пишу `src/model.rs` с типами, RED-заглушками и падающими тестами, затем обновляю `src/lib.rs`.



Теперь проверяю acceptance-скрипт — сборка OK, компиляция тестов OK, запуск падает.



There are warnings about unused import. Let me fix them:



One more warning — unused import. Let me remove `FromStr`:



Clean build. Now let's run the acceptance check:



Все работает точно по spec. BUILD OK → тесты компилируются → прогон падает на assert (NotImplemented/0 для формулы и загрузок), exit code 0.

STATUS: RED
FILES: crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/model.rs
TEST: cargo test -p stellwerk-registry model
EVIDENCE: exit 0, вывод:

```
running 8 tests
test model::tests::from_toml_str_parses_fixture ... FAILED
test model::tests::from_toml_str_rejects_unknown_module_keys ... ok
test model::tests::from_toml_str_rejects_unknown_source_keys ... ok
test model::tests::source_roundtrip_git ... ok
test model::tests::source_roundtrip_crate ... ok
test model::tests::tokens_for_formula ... FAILED
test model::tests::load_reads_file ... FAILED
test model::tests::source_roundtrip_npm ... ok

test result: FAILED. 5 passed; 3 failed; 0 ignored; 0 measured

---- model::tests::from_toml_str_parses_fixture stdout ----
thread 'model::tests::from_toml_str_parses_fixture' panicked at model.rs:145: fixture must parse: NotImplemented

---- model::tests::tokens_for_formula stdout ----
thread 'model::tests::tokens_for_formula' panicked at model.rs:202: assertion `left == right` failed
  left: 0
 right: 813

---- model::tests::load_reads_file stdout ----
thread 'model::tests::load_reads_file' panicked at model.rs:195: load must succeed: NotImplemented
```

3 падающих теста (RED): from_toml_str, tokens_for, load — заглушки возвращают NotImplemented/0. Сердесериализация и deny_unknown_fields уже работают корректно (5 passed). Green сделается в packet-02.

---
first_step context_input: 0
total tokens: 11820
