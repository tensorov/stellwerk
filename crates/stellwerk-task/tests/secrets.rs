//! Спецификация M5/packet-09: секрет живёт в прокси, и путей к нему у агента нет.
//!
//! Проверяется не «мы ничего не передавали», а перечисление: окружение собрано из белого
//! списка и не содержит имени токена, аргументы значения не несут, конфигурация прокси вне
//! песочницы (и это доказывает ядро, с контролем без песочницы), а прокси возвращает тело
//! без значения токена — по отказу в том числе.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::secrets::{
    agent_argv, read_token_outside, AgentEnv, ProxyError, SecretError, Token, TokenProxy,
};

const BUDGET: Duration = Duration::from_secs(30);
const TOUCH: [&str; 2] = ["/usr/bin/touch", "/bin/touch"];
/// Имя, под которым токен известен системе: именно его не должно быть в окружении агента.
const NAME: &str = "GITHUB_TOKEN";
/// Значение токена — константой, а не вызовом: тест ищет именно эту строку, и строка,
/// собранная функцией, искала бы сама себя.
const VALUE: &str = "ghp-t0ken-never-in-the-agent-reach";
const URL: &str = "https://api.github.com/user";

fn token() -> Token {
    Token::new(NAME, VALUE).expect("имя и значения заданы")
}

/// Хостовое окружение списком пар: ровно та форма, из которой собирается окружение агента.
fn host(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|kv| (kv.0.to_owned(), kv.1.to_owned()))
        .collect()
}

/// Системные каталоги — только на чтение. Список ДАН, а не угадан: `--ro-bind` на
/// отсутствующий источник `bwrap` считает ошибкой.
fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib64", "/etc"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

/// Программа берётся из системных каталогов: `/usr/bin/…` и `/bin/…` — одно и то же.
/// Молчаливый `None` означал бы «проверять нечем».
fn in_system(names: &[&str]) -> PathBuf {
    let found = names.iter().map(PathBuf::from).find(|p| p.is_file());
    found.unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

/// Каталог фикстуры под текущим процессом: имя по тегу, чтобы прогоны не делили одну копию.
fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-p09-{tag}-{}", std::process::id()))
}

/// п. 1 и закон 6: отсутствующая переменная остаётся отсутствующей, а не превращается в
/// `NAME=`. Иначе «не настроено» и «настроено в пустоту» читались бы одним словом.
#[test]
fn an_allowlisted_variable_the_host_lacks_stays_absent_and_never_becomes_a_blank_value() {
    let t = token();
    // Токен лежит у хоста под чужим именем и в белый список не входит: белый список —
    // перечень того, что ЕСТЬ, а не фильтр, и «не перечислили» значит «не попало».
    let given = host(&[("PATH", "/usr/bin"), ("HOME", ""), ("CI_JOB_TOKEN", VALUE)]);
    let env = AgentEnv::assemble(&["PATH", "LANG", "HOME"], &given, &t).expect("собирается");
    assert_eq!(env.absent, ["LANG".to_owned()]);
    assert_eq!(env.value("LANG"), None);
    // Пустое значение, ДАННОЕ хостом, — данные («задано, пусто»), а не отказ и не
    // подстановка: стереть различие между ним и отсутствием значит выдумать данные.
    assert_eq!(env.value("HOME"), Some(""));
    assert!(!env.reaches(&t));
}

/// Приёмка 2: попытка передать токен через окружение агента отвергается ДО запуска, и отказ
/// называет имя переменной, а не значение.
#[test]
fn an_attempt_to_hand_the_token_through_the_agent_environment_is_refused() {
    let t = token();
    let refused = AgentEnv::assemble(&[NAME], &host(&[(NAME, VALUE)]), &t);
    let told = refused.expect_err("токен в окружении — отказ");
    match told {
        SecretError::TokenInEnvironment { name } => assert_eq!(name, NAME),
        other => panic!("токен в окружении — отказ, а не {other:?}"),
    }
    // Отказ и по ЗНАЧЕНИЮ под чужим именем: белый список проверяет имена, а секрет
    // проверяется по существу, и переименование его не обезличивает.
    let alias = AgentEnv::assemble(&["PATH"], &host(&[("PATH", VALUE)]), &t);
    assert!(matches!(alias, Err(SecretError::TokenInEnvironment { .. })));
    // Повтор в белом списке — два утверждения об одном имени, и «взяли последнее» стёрло бы
    // различие между ними молча.
    let twice = AgentEnv::assemble(&["PATH", "PATH"], &[], &t);
    let told = twice.expect_err("повтор — отказ");
    assert!(matches!(told, SecretError::RepeatedName { .. }));
}

/// п. 3: аргументы команды видны в списке процессов любому, поэтому токен в них — это не
/// «секрет в аргументах», а публикация. Отказ ДО сборки вектора.
#[test]
fn a_token_in_a_command_argument_is_refused_and_the_vector_is_never_built() {
    let t = token();
    // Номер — позиция в векторе, а не в хвосте аргументов: в списке процессов видно
    // именно `argv[2]`, и отказ должен указывать на то, что там стоит.
    let asked = agent_argv(&t, "pi", &["--run", VALUE]).expect_err("значение токена — отказ");
    assert!(matches!(asked, SecretError::TokenInArguments { at: 2 }));
    // `argv[0]` — тоже аргумент: программа читается в списке процессов так же.
    let first = agent_argv(&t, VALUE, &[]).expect_err("программа — тоже аргумент");
    assert!(matches!(first, SecretError::TokenInArguments { at: 0 }));
    let clean = agent_argv(&t, "pi", &["--run", "task-1"]).expect("чистые аргументы");
    assert_eq!(clean, ["pi", "--run", "task-1"]);
}

/// п. 2: прокси подставляет токен сам — это видно на проводе, — и возвращает тело, в котором
/// токена нет, даже когда внешний сервис отразил заголовок в ответе.
#[test]
fn the_proxy_substitutes_the_token_and_returns_a_body_that_does_not_carry_it() {
    let proxy = TokenProxy::new(token());
    let sent = proxy.send("GET", URL, |wire| {
        assert!(
            wire.contains(&format!("authorization: Bearer {VALUE}")),
            "{wire}"
        );
        Ok((200, format!("{{\"ok\":true,\"echo\":\"{wire}\"}}")))
    });
    let response = sent.expect("прокси отвечает телом");
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"ok\":true"));
    // Смывка на успехе ровно та же, что на отказе: «вряд ли сервис отразит заголовок» — не
    // доказательство, а «не должно случиться» в журнале выглядит как «случилось».
    assert!(
        !response.body.contains(VALUE),
        "тело ответа: {}",
        response.body
    );
    assert!(!response.body.to_lowercase().contains("authorization"));
}

/// п. 4: причина отказа может содержать эхо заголовка авторизации — смывается и она, иначе
/// отказ стал бы тем самым каналом утечки, который запрещён.
#[test]
fn a_proxy_failure_whose_cause_echoes_the_authorization_header_carries_no_token() {
    let proxy = TokenProxy::new(token());
    let failed = proxy.send("GET", URL, |wire| {
        Err(format!("reset while sending {wire}"))
    });
    let refused = match failed {
        Err(ProxyError::Upstream { reason }) => reason,
        Ok(other) => panic!("ошибка транспорта — отказ, а не {other:?}"),
    };
    // Проверяются и текст для человека, и `Debug`: отказ всплывает и в сообщение об ошибке.
    let shown = format!("{refused}{refused:?}");
    assert!(!shown.contains(VALUE), "отказ без токена: {shown}");
    assert!(
        !shown.to_lowercase().contains("authorization"),
        "отказ без эха: {shown}"
    );
}

/// Пустое значение токена — отсутствие данных, а не «токен, равный пустоте»: такой смывался
/// бы из диагностики целиком и вернулся бы оттуда обратным словом.
#[test]
fn a_blank_token_is_refused_and_its_debug_output_carries_no_value() {
    let blank = Token::new(NAME, "  ").expect_err("пустое значение — отказ");
    assert!(matches!(blank, SecretError::BlankToken { field: "value" }));
    let unnamed = Token::new(" ", VALUE).expect_err("пустое имя — отказ");
    assert!(matches!(unnamed, SecretError::BlankToken { field: "name" }));
    // `derive(Debug)` напечатал бы значение в любом сообщении об отказе — поэтому руками.
    let shown = format!("{:?}", token());
    assert!(!shown.contains(VALUE), "{shown}");
}

/// п. 5: конфигурация прокси вне песочницы агента. Фикстура лежит в каталоге, который НЕ
/// является корнем ни одного bind-назначения: под таким корнем `bwrap` создал бы её заново в
/// записываемом слое, и недостижимость доказывала бы ровно ничего.
#[test]
fn the_proxy_configuration_is_unreachable_from_the_sandbox_and_the_kernel_denies_it() {
    let root = fixture_root("cfg");
    let (work, tmp) = (root.join("work"), root.join("tmp"));
    let config = fixture_root("cfg-outside").join("proxy.conf");
    for dir in [&work, &tmp, config.parent().expect("каталог конфигурации")] {
        fs::create_dir_all(dir).expect("каталог фикстуры создан");
    }
    fs::write(&config, VALUE).expect("конфигурация записана");
    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only: system_dirs(),
        window: Duration::from_secs(60),
    };
    // Всё, что агент видит: рабочая копия, каталог временных файлов и системные каталоги.
    let mut visible = vec![work.clone(), tmp];
    visible.extend(sandbox.read_only.iter().cloned());
    let touch = in_system(&TOUCH);
    // Настоящий запуск: `bwrap` обязана быть в системе, и её отсутствие — падение теста, а
    // не повод объявить границу проверенной.
    let run = |target: &Path| -> Output {
        let args = [target.to_str().expect("путь из temp_dir валиден")];
        let mut cmd = sandbox
            .command(&work, &touch, &args, BUDGET)
            .expect("запуск разрешён");
        cmd.output()
            .expect("нет `bwrap` — нет и проверенной границы")
    };

    // Контроль БЕЗ песочницы: конфигурация доступна на хосте, иначе «отказ» дала бы фикстура.
    let plain = Command::new(&touch)
        .arg(&config)
        .output()
        .expect("контроль запускается");
    assert!(plain.status.success(), "{plain:?}");
    fs::remove_file(&config).expect("фикстура убрана, иначе отказ не отличить от «есть»");

    // Копия конфигурации ПОД рабочим каталогом видна агенту: это bind с `--bind`. Значит
    // проверка границы не суеверие, и под корнем bind'а конфигурации быть не может.
    let inner = work.join("proxy.conf");
    fs::write(&inner, VALUE).expect("внутренняя копия записана");
    assert!(
        run(&inner).status.success(),
        "под корнем bind'а файл виден агенту"
    );
    let inside = read_token_outside(&inner, &visible, NAME).expect_err("под bind'ом — отказ");
    assert_eq!(
        inside,
        SecretError::ConfigInsideSandbox {
            root: work.clone(),
            path: inner
        }
    );

    // Настоящий отказ ядра: у процесса в namespace нет ни этого пути, ни его каталога.
    let denied = run(&config);
    assert!(!denied.status.success(), "{denied:?}");
    assert!(!config.exists());
    // И вне песочницы конфигурация читается — иначе «отказ» не отличился бы от «нет файла».
    // Файл возвращают на диск ПОСЛЕ отказа ядра: пока он есть, отказ был бы «не читается»,
    // а не «пути нет», и это другой инцидент.
    fs::write(&config, VALUE).expect("конфигурация возвращена");
    let read = read_token_outside(&config, &visible, NAME).expect("внешняя читается");
    assert!(
        !format!("{read:?}").contains(VALUE),
        "чтение не печатает токен"
    );
}
