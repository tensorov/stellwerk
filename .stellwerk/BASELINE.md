# BASELINE — мутационный baseline на коде `10ffec4`

Пакет: M8-01. Дата измерения: 2026-10-11.
Ревизия: `10ffec470d18a3a9d96768bf2091c1cc8c83cb48` (`main` после отката).
Машина: x99, 40 ядер. `cargo-mutants 27.1.0`.
Артефакты: `~/gits/verify/m8-{task,gateway,core}/mutants.out/outcomes.json` (на x99, `/home`).
`TMPDIR=~/gits/scratch-m8` (вне репозитория, `/home`).

Числа ниже взяты из `outcomes.json` (поля `total_mutants`/`caught`/`missed`/`timeout`/`unviable`),
а не из stdout. Список missed — из `mutants.out/missed.txt`.

## Гигиена прогона

- `git status --porcelain` на x99 перед стартом: **пуст** (проверено; HEAD = `10ffec4`).
- Живых `rustc` перед каждым стартом: **0**.
- Один `cargo-mutants` за раз; следующий запускался только после `pgrep -c cargo-mutants == 0`.
- `--jobs 18`, `--timeout 300`, `--baseline run` (по умолчанию).
- Load (1/5/15 мин) на момент старта каждого прогона:
  - task: `0.35 / 0.29 / 0.33`
  - gateway: `1.25 / 53.69 / 53.54` (5-мин среднее ещё затухало после task; 1-мин = 1.25)
  - core: `0.40 / 15.78 / 36.16`

## Сводка

| Пакет | total | caught | missed | timeout | unviable |
|---|---|---|---|---|---|
| `stellwerk-task` | 394 | 312 | 40 | 0 | 42 |
| `stellwerk-gateway` | 269 | 234 | 14 | 0 | 21 |
| `stellwerk-core` | 42 | 36 | 3 | 0 | 3 |
| **Итого** | **705** | **582** | **57** | **0** | **66** |

Совпадает с ожиданием плана для task (394/40) и gateway (269/14). `core` измерен впервые.

---

## 1. `stellwerk-task`

Команда:

```
cd ~/gits/stellwerk && TMPDIR=~/gits/scratch-m8 \
  cargo mutants --package stellwerk-task --jobs 18 --timeout 300 \
  --output ~/gits/verify/m8-task
```

`outcomes.json`: `total_mutants=394, caught=312, missed=40, timeout=0, unviable=42`.
Прогон: `2026-10-11T07:48:22Z` → `07:56:49Z` (≈8m27s). Load на старте: `0.35 / 0.29 / 0.33`.
Строка stdout: `394 mutants tested in 8m: 40 missed, 312 caught, 42 unviable`.

### Missed (40)

```
crates/stellwerk-task/src/approval.rs:205:20: replace - with + in rfc3339_utc
crates/stellwerk-task/src/approval.rs:205:34: replace + with - in rfc3339_utc
crates/stellwerk-task/src/approval.rs:205:34: replace + with * in rfc3339_utc
crates/stellwerk-task/src/approval.rs:205:49: replace - with + in rfc3339_utc
crates/stellwerk-task/src/approval.rs:205:49: replace - with / in rfc3339_utc
crates/stellwerk-task/src/approval.rs:207:42: replace - with + in rfc3339_utc
crates/stellwerk-task/src/approval.rs:208:23: replace + with - in rfc3339_utc
crates/stellwerk-task/src/approval.rs:210:19: replace < with == in rfc3339_utc
crates/stellwerk-task/src/approval.rs:210:19: replace < with <= in rfc3339_utc
crates/stellwerk-task/src/approval.rs:210:19: replace < with > in rfc3339_utc
crates/stellwerk-task/src/approval.rs:210:45: replace - with + in rfc3339_utc
crates/stellwerk-task/src/approval.rs:210:45: replace - with / in rfc3339_utc
crates/stellwerk-task/src/approval.rs:211:30: replace + with * in rfc3339_utc
crates/stellwerk-task/src/approval.rs:211:30: replace + with - in rfc3339_utc
crates/stellwerk-task/src/evidence.rs:146:5: replace kind_name -> &'static str with ""
crates/stellwerk-task/src/evidence.rs:146:5: replace kind_name -> &'static str with "xyzzy"
crates/stellwerk-task/src/evidence.rs:157:5: replace sha256_hex -> String with String::new()
crates/stellwerk-task/src/evidence.rs:157:5: replace sha256_hex -> String with "xyzzy".into()
crates/stellwerk-task/src/m5/mod.rs:189:56: replace < with <= in <impl EngineAdapter for FixtureEngine>::session
crates/stellwerk-task/src/m5/mod.rs:235:38: replace & with | in executable
crates/stellwerk-task/src/m5/mod.rs:235:38: replace & with ^ in executable
crates/stellwerk-task/src/runner.rs:215:9: delete match arm "error" in stop_reason_of
crates/stellwerk-task/src/runner.rs:216:9: delete match arm "aborted" in stop_reason_of
crates/stellwerk-task/src/runner.rs:233:37: replace > with >= in verdict_for
crates/stellwerk-task/src/runner.rs:237:18: replace > with >= in verdict_for
crates/stellwerk-task/src/runner.rs:243:18: replace > with >= in verdict_for
crates/stellwerk-task/src/runner.rs:253:5: replace gate -> u32 with 1
crates/stellwerk-task/src/sandbox.rs:113:9: replace Sandbox::is_kept -> bool with true
crates/stellwerk-task/src/sandbox.rs:113:9: replace Sandbox::is_kept -> bool with false
crates/stellwerk-task/src/sandbox.rs:120:5: replace default_script -> PathBuf with Default::default()
crates/stellwerk-task/src/sandbox.rs:126:5: replace keep_requested -> bool with true
crates/stellwerk-task/src/sandbox.rs:126:5: replace keep_requested -> bool with false
crates/stellwerk-task/src/sandbox.rs:126:49: delete ! in keep_requested
crates/stellwerk-task/src/sandbox.rs:126:63: replace && with || in keep_requested
crates/stellwerk-task/src/sandbox.rs:126:68: replace != with == in keep_requested
crates/stellwerk-task/src/taskfile.rs:93:9: replace TaskFile::validate -> Result<(), TaskFileError> with Ok(())
crates/stellwerk-task/src/m5/sandbox_bwrap.rs:121:19: replace > with >= in Sandbox::preflight
crates/stellwerk-task/src/m5/secrets.rs:108:9: replace <impl fmt::Debug for Token>::fmt -> fmt::Result with Ok(Default::default())
crates/stellwerk-task/src/m5/secrets.rs:172:9: replace AgentEnv::reaches -> bool with false
crates/stellwerk-task/src/m5/secrets.rs:174:44: replace || with && in AgentEnv::reaches
```

По файлам: `approval.rs` 14, `evidence.rs` 4, `m5/mod.rs` 3, `runner.rs` 6,
`sandbox.rs` 8, `taskfile.rs` 1, `m5/sandbox_bwrap.rs` 1, `m5/secrets.rs` 3.

---

## 2. `stellwerk-gateway`

Команда:

```
cd ~/gits/stellwerk && TMPDIR=~/gits/scratch-m8 \
  cargo mutants --package stellwerk-gateway --jobs 18 --timeout 300 \
  --output ~/gits/verify/m8-gateway
```

`outcomes.json`: `total_mutants=269, caught=234, missed=14, timeout=0, unviable=21`.
Прогон: `2026-10-11T08:01:50Z` → `08:03:40Z` (≈1m50s). Load на старте: `1.25 / 53.69 / 53.54`.
Строка stdout: `269 mutants tested in 2m: 14 missed, 234 caught, 21 unviable`.

### Missed (14)

```
crates/stellwerk-gateway/src/session.rs:227:28: replace match guard expired(&asked.expires_at) with true in Session::suspend
crates/stellwerk-gateway/src/session.rs:321:31: replace || with && in Session::tokens_per_byte
crates/stellwerk-gateway/src/session.rs:328:55: replace || with && in Session::tokens_per_byte
crates/stellwerk-gateway/src/session.rs:373:63: replace == with != in mount_refusal
crates/stellwerk-gateway/src/session.rs:392:5: replace call_id -> String with "xyzzy".into()
crates/stellwerk-gateway/src/mount.rs:134:17: replace > with >= in plan
crates/stellwerk-gateway/src/shaping.rs:59:37: replace || with && in shape
crates/stellwerk-gateway/src/shaping.rs:66:39: replace * with + in shape
crates/stellwerk-gateway/src/shaping.rs:80:36: replace / with * in shape
crates/stellwerk-gateway/src/shaping.rs:112:15: replace > with < in char_boundary_prefix
crates/stellwerk-gateway/src/shaping.rs:112:15: replace > with == in char_boundary_prefix
crates/stellwerk-gateway/src/shaping.rs:112:15: replace > with >= in char_boundary_prefix
crates/stellwerk-gateway/src/shaping.rs:113:13: replace -= with /= in char_boundary_prefix
crates/stellwerk-gateway/src/shaping.rs:113:13: replace -= with += in char_boundary_prefix
```

По файлам: `session.rs` 5, `mount.rs` 1, `shaping.rs` 8.

---

## 3. `stellwerk-core` — первый прогон за историю репозитория

Команда:

```
cd ~/gits/stellwerk && TMPDIR=~/gits/scratch-m8 \
  cargo mutants --package stellwerk-core --jobs 18 --timeout 300 \
  --output ~/gits/verify/m8-core
```

`outcomes.json`: `total_mutants=42, caught=36, missed=3, timeout=0, unviable=3`.
Прогон: `2026-10-11T08:10:27Z` → `08:11:01Z` (≈34s). Load на старте: `0.40 / 15.78 / 36.16`.
Строка stdout: `42 mutants tested in 34s: 3 missed, 36 caught, 3 unviable`.

Ожиданий не было (core не мутировали ни разу). Все 42 мутанта — в `contract.rs`.

### Missed (3)

```
crates/stellwerk-core/src/contract.rs:222:13: delete match arm (TaskStatus::Done, _) |(TaskStatus::Failed, _) in TaskStatus::can_transition_to
crates/stellwerk-core/src/contract.rs:225:26: replace match guard s == next with false in TaskStatus::can_transition_to
crates/stellwerk-core/src/contract.rs:242:13: delete match arm (TaskStatus::Suspended, TaskStatus::EvidenceSubmitted) in TaskStatus::can_transition_to
```

---

## Кандидаты в эквивалентные

Признак: тест проходит и на оригинале, и на мутанте, и **никакой** тест не различит
поведение. Ниже — только те, для которых различие не наблюдаемо для всех входов
(либо, для одного, — только в недостижимой точке). Всё остальное из списков missed
остаётся `missed`: его можно убить тестом, он просто не покрыт.

### Доказанно эквивалентные (различие не наблюдаемо ни на одном входе)

1. **`crates/stellwerk-task/src/runner.rs:237:18`** — `min_steps > 0` → `min_steps >= 0`.
   `min_steps: u32`, поэтому `>= 0` всегда истинно; при `min_steps == 0` второй конъюнкт
   `tool_steps < min_steps` = `tool_steps < 0` тоже ложен для `u32`, и обе версии пропускают
   проверку — результат `verdict_for` совпадает на всех входах.

2. **`crates/stellwerk-gateway/src/shaping.rs:112:15`** — `cut > 0` → `cut >= 0`.
   `cut: usize`, поэтому `>= 0` всегда истинно; при `cut == 0` второй конъюнкт
   `!text.is_char_boundary(0)` ложен (индекс 0 — всегда граница символа), и цикл завершается
   так же, как в оригинале, — `char_boundary_prefix` возвращает тот же срез.

3. **`crates/stellwerk-core/src/contract.rs:222:13`** — удаление ветки
   `(Done, _) | (Failed, _) => false`.
   Для `(Done, Done)`/`(Failed, Failed)` срабатывает охран `(s, next) if s == next => false`
   (строка 225), для `(Done, X)`/`(Failed, X)` при `X != self` — хвост `_ => false`
   (строка 264); обе ветки дают `false`, как и удалённая, — `can_transition_to` не меняется.

4. **`crates/stellwerk-core/src/contract.rs:225:26`** — охран `s == next` → `false`.
   Ни одна из явных веток `match` не является переходом в себя, поэтому все `(s, s)` и так
   попадают в хвост `_ => false` (строка 264), а `(Done, Done)`/`(Failed, Failed)` ловит
   ветка 222; отключение охрана не меняет ни одного результата.

### Пограничный (план называет эквивалентным; различие — только в недостижимой точке)

5. **`crates/stellwerk-task/src/m5/mod.rs:189:56`** — `Instant::now() < deadline` →
   `Instant::now() <= deadline`.
   Различие возможно только при точном равенстве `Instant::now() == deadline` (монотонные
   часы с наносекундным разрешением — недостижимая точка), и даже тогда пост-цикловая
   проверка `if !finished(&mut child)?` даёт тот же исход (`Ok` либо `TimedOut`), отличаясь
   лишь на один лишний опрос в 2 мс. План (`M8-refactoring-debt-first.md`, часть 3) уже
   называет этот мутант равнозначным; здесь он оставлен кандидатом, а не закрытым.

### План называл эквивалентным, но анализ это опровергает — остаётся `missed`

- **`crates/stellwerk-task/src/sandbox.rs:120:5`** — `default_script -> PathBuf` →
  `Default::default()` (пустой `PathBuf`).
  `default_script` вызывается только при незаданном `STELLWERK_SANDBOX` (строка 52), и
  подстановка пустого пути меняет исход `Sandbox::prepare` (пустой путь → `fs::File::open("")`
  → `SandboxError::Spawn` вместо запуска `scripts/worker/sandbox.sh`); тест, снявший
  `STELLWERK_SANDBOX` или вызвавший `default_script()` напрямую, различит мутанта. Значит,
  он убиваем и **не** эквивалентен — оставлен в списке missed.

## Таймауты

Таймаутов нет ни в одном из трёх прогонов (`timeout = 0` в каждом `outcomes.json`,
`mutants.out/timeout.txt` пуст). Load на старте каждого прогона приведён выше; во время
прогонов load поднимался (до ~193 при `--jobs 18`), но порог `--timeout 300` не был
достигнут ни одним тестом.

## Что этот baseline НЕ утверждает

- Зелёный/красный `cargo test` не измерялся и результатом пакета не является.
- Числа относятся к коду `10ffec4`; после любого рефакторинга части 2 плана M8 их нужно
  пересчитывать (в частности, копия `rfc3339_utc` в `approval.rs` и её 14 мутантов
  исчезнут при переносе в `core`).
- Категория `equivalent` здесь — только кандидаты для реестра диспозиций (M8-N), а не
  закрытые мутанты; гейт их пока не исключает.
