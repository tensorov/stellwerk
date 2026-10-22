//! # Секрет живёт в прокси: у процесса агента нет ни одного пути к токену
//!
//! Агент — код, который пишет модели, а модель подсказывается. Значит секрет в зоне
//! достижимости агента равен публикации секрета, а «мы его не передаём» доводом не
//! является: достаточно одного пути, который никто не перечислил.
//!
//! Поэтому проверка здесь не «мы не передали», а перечисление и отказ по каждому пункту:
//! окружение собрано из белого списка, аргументы команды проверены на значение, конфигурация
//! прокси лежит вне песочницы, а сам прокси отдаёт тело ответа — без заголовка авторизации и
//! без значения токена, по успеху так же, как по отказу.

use std::fmt;
use std::fs;
use std::iter::once;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// Замена вместо значения. В выводе видна как «здесь было», и от секрета неотличима.
pub const REDACTED: &str = "<смыто>";

/// Отказ на границе секрета. Варианты не сливаются: «имя пустое», «повтор в белом списке»,
/// «токен в окружении», «токен в аргументах», «конфигурация в песочнице» и «файл не
/// прочитан» чинятся по-разному, и молчаливое слияние отдало бы «нет данных» под видом
/// отказа (закон 6).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretError {
    /// Поле токена пустое. Пустая строка — это «задано, пусто», а не «не задано», и
    /// подставлять её дальше нельзя: пустое значение смывается из диагностики целиком и
    /// вернулось бы оттуда обратным словом.
    #[error("token {field} is blank: an empty value is a value, and absence is not one")]
    BlankToken { field: &'static str },
    /// Имя повторено в белом списке: два утверждения об одном имени, и «взяли последнее»
    /// стёрло бы различие между ними молча.
    #[error("`{name}` is listed twice in the agent allowlist: two claims, one name")]
    RepeatedName { name: String },
    /// Токен дошёл бы до окружения агента — по имени или по значению.
    #[error("`{name}` would hand the external token to the agent: refused before launch")]
    TokenInEnvironment { name: String },
    /// Токен в аргументах команды: список процессов машины читает любой пользователь.
    #[error("argument {at} carries the external token: the process list is public")]
    TokenInArguments { at: usize },
    /// Конфигурация прокси лежит в зоне, видимой агенту, — то есть это файл песочницы.
    #[error("proxy configuration `{path}` sits inside `{root}`: it must stay outside the sandbox")]
    ConfigInsideSandbox { root: PathBuf, path: PathBuf },
    /// Файл не прочитан. Отдельный вариант: «нет файла» и «нет права» — разные инциденты, а
    /// «открылось пустое» не было бы ни тем, ни другим.
    #[error("proxy configuration `{path}` was not read: {why}")]
    Unreadable { path: PathBuf, why: String },
    /// Поле запроса пустое. Имя поля в отказе: «неверный запрос» без него чинится чтением
    /// исходника, а не сообщением.
    #[error("request field `{field}` is blank: an empty value is not an address")]
    EmptyRequestField { field: &'static str },
    /// Перевод строки в методе, хосте, пути или типе — это попытка дописать заголовок.
    /// Проверка ДО сборки провода: после `authorization` дописанный заголовок был бы принят
    /// сервисом как часть запроса, и отказ опоздал бы.
    #[error("request field `{field}` carries a line break: a header would be injected")]
    ControlByte { field: &'static str },
    /// Путь относительный: без ведущего `/` запрос читается как абсолютная форма прокси, и
    /// сервис получил бы чужой адрес в строке запроса.
    #[error("request path is not absolute: it must start with `/`")]
    PathNotAbsolute,
    /// Тело без типа: сервис разобрал бы его по догадке, а «непонятый запрос» неотличим от
    /// «мы собрали его неверно».
    #[error("request carries a body without a content type: the service would have to guess")]
    BodyWithoutContentType,
    /// Тип без тела: утверждение о данных, которых нет. Отдельный вариант, потому что это
    /// ошибка сборки запроса, а не «тело пустое».
    #[error("request names a content type but carries no body")]
    ContentTypeWithoutBody,
    /// Тело сверх потолка. Размер и потолок в отказе: «слишком большое» без числа не даёт
    /// понять, насколько.
    #[error("request body is {len} bytes, over the {max}-byte limit")]
    BodyTooLarge { len: usize, max: usize },
}

/// Токен. `Debug` написан руками и печатает длину, а не значение: `derive(Debug)` вёл бы
/// токен в любое сообщение об отказе, а это ровно тот канал, который запрещён. `Display` нет
/// вовсе и публичного доступа к значению тоже: наружу значение уходит единственным
/// способом — подстановкой прокси в заголовок, который наружу не возвращается.
#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    name: String,
    value: String,
}

impl Token {
    /// Имя и значение обязательны: пустому имени не на что опереться в отказе, а пустое
    /// значение — это «задано, пусто», и читалось бы как настоящий токен.
    pub fn new(name: &str, value: &str) -> Result<Self, SecretError> {
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            return Err(SecretError::BlankToken { field: "name" });
        }
        if value.is_empty() {
            return Err(SecretError::BlankToken { field: "value" });
        }
        Ok(Self {
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }

    /// Есть ли значение в тексте. Проверка по ЗНАЧЕНИЮ, а не по имени: имя токена в
    /// аргументе или в теле запроса секретом не является, и отказ по нему был бы ложным.
    fn has_value(&self, text: &str) -> bool {
        text.contains(&self.value)
    }

    /// Смывка. Сперва имя заголовка вместе с остатком строки — до конца, а не до значения:
    /// эхо заголовка остаётся эхом и тогда, когда токен стёрт, а «здесь был заголовок
    /// авторизации» в чужом парсере всё равно разбирается как заголовок. Затем само
    /// значение — во всём остатке: токен без заголовка уже утечка, а порядок проходов
    /// значим, потому что второй идёт по тому, что первый не нашёл.
    fn redact(&self, text: &str) -> String {
        let by_header: Vec<String> = text
            .split('\n')
            .map(
                |line| match line.to_ascii_lowercase().find("authorization") {
                    // Сдвиг байтов совпадает: `to_ascii_lowercase` меняет байт на байт и длину
                    // не трогает, поэтому `at` из lowercased строки годен и для исходной.
                    Some(at) => format!("{}{REDACTED}", &line[..at]),
                    None => line.to_owned(),
                },
            )
            .collect();
        by_header.join("\n").replace(&self.value, REDACTED)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.value.chars().count();
        write!(f, "Token({}, {len} симв.)", self.name)
    }
}

/// Собранное окружение агента: выданные переменные и — ОТДЕЛЬНО — те, которых нет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEnv {
    granted: Vec<(String, String)>,
    /// Имена, которых нет ни у хоста, ни в выдаче. Список, а не счётчик: «не настроено
    /// три штуки» и «настроено неверно» чинятся по-разному.
    pub absent: Vec<String>,
}

impl AgentEnv {
    /// Сборка из белого списка, а не из окружения хоста: «всё, кроме запрещённого» — это
    /// список запретов, а список запретов уезжает в переменную, добавленную позже.
    pub fn assemble(
        allowlist: &[&str],
        host: &[(String, String)],
        token: &Token,
    ) -> Result<Self, SecretError> {
        let mut granted: Vec<(String, String)> = Vec::new();
        let mut absent: Vec<String> = Vec::new();
        for name in allowlist {
            let claimed = granted.iter().any(|(n, _)| n.as_str() == *name)
                || absent.iter().any(|n| n.as_str() == *name);
            if claimed {
                return Err(SecretError::RepeatedName {
                    name: (*name).to_owned(),
                });
            }
            match host.iter().find(|(n, _)| n.as_str() == *name) {
                Some((given, value)) => {
                    // Отказ по имени И по значению: белый список проверяет имена, а секрет
                    // проверяется по существу — переименование его не обезличивает.
                    if given == &token.name || token.has_value(value) {
                        return Err(SecretError::TokenInEnvironment {
                            name: (*name).to_owned(),
                        });
                    }
                    granted.push(((*name).to_owned(), value.clone()));
                }
                // Отсутствие — своё состояние, а не пустое значение по умолчанию: `NAME=`
                // читается как «задано, но пусто», и тогда «не настроено» и «настроено в
                // пустоту» стали бы одним словом (закон 6).
                None => absent.push((*name).to_owned()),
            }
        }
        Ok(Self { granted, absent })
    }

    /// Значение переменной; `None` — переменной нет. Различие «нет» и «пусто» и есть
    /// содержание типа: `Some("")` — задано и пусто, `None` — не задано.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.granted
            .iter()
            .find(|(n, _)| n.as_str() == name)
            .map(|(_, v)| v.as_str())
    }

    /// Ни одного пути: ни по имени, ни по значению. Пустой ответ — единственное
    /// доказательство, которого стоит требовать вместо «мы ничего не передавали».
    pub fn reaches(&self, token: &Token) -> bool {
        self.granted
            .iter()
            .any(|(n, v)| n == &token.name || token.has_value(v))
    }
}

/// Аргументы агентного процесса. Список процессов машины читает любой пользователь, и токен
/// в аргументе — это публикация, а не «секрет, который не выводили». Проверка значения идёт
/// ДО сборки вектора: `execve` необратим, а список процессов переживает сам процесс.
pub fn agent_argv(token: &Token, program: &str, args: &[&str]) -> Result<Vec<String>, SecretError> {
    for (at, arg) in once(program).chain(args.iter().copied()).enumerate() {
        if token.has_value(arg) {
            return Err(SecretError::TokenInArguments { at });
        }
    }
    let mut argv = vec![program.to_owned()];
    argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    Ok(argv)
}

/// Ответ, который уходит к агенту: код и тело. Ни заголовка авторизации, ни значения токена
/// в структуре нет — и `Debug` здесь тоже, потому что тело приходит извне: его смывка не
/// украшение, а единственная причина, по которой поле вообще можно отдавать наружу. Поля
/// открыты намеренно: читать их не опасно ровно потому, что тело прошло смывку.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyResponse {
    /// Код ответа сервиса: данные, а не выдумка прокси.
    pub status: u16,
    /// Тело ответа. Уже прошедшее смывку: агент читает его и не имеет дела с заголовком.
    pub body: String,
}

/// Потолок тела запроса. Одно число для проверки и для теста: литерал в двух местах
/// разошёлся бы молча.
pub const MAX_BODY: usize = 1024 * 1024;

/// Запрос, который прокси отправит. Поля закрыты, а конструктор проверяет их все: значение,
/// не прошедшее проверку, не должно дожить до транспорта, потому что отправку не отменить.
/// Поля для заголовка авторизации здесь нет и быть не может — его вставляет только прокси.
#[derive(Debug, Clone, Copy)]
pub struct ProxyRequest<'a> {
    method: &'a str,
    host: &'a str,
    path: &'a str,
    content_type: Option<&'a str>,
    body: Option<&'a str>,
}

impl<'a> ProxyRequest<'a> {
    /// Все поля обязательны, кроме типа и тела, которые ходят парой. Пустое тело с типом —
    /// данные («задано, пусто»), тип без тела — ошибка сборки: это разные вещи.
    pub fn new(
        method: &'a str,
        host: &'a str,
        path: &'a str,
        content_type: Option<&'a str>,
        body: Option<&'a str>,
    ) -> Result<Self, SecretError> {
        check_line("method", method)?;
        check_line("host", host)?;
        check_line("path", path)?;
        if !path.starts_with('/') {
            return Err(SecretError::PathNotAbsolute);
        }
        match (content_type, body) {
            (None, Some(_)) => return Err(SecretError::BodyWithoutContentType),
            (Some(_), None) => return Err(SecretError::ContentTypeWithoutBody),
            (Some(ct), Some(body)) => {
                check_line("content-type", ct)?;
                if body.len() > MAX_BODY {
                    return Err(SecretError::BodyTooLarge {
                        len: body.len(),
                        max: MAX_BODY,
                    });
                }
            }
            (None, None) => {}
        }
        Ok(Self {
            method,
            host,
            path,
            content_type,
            body,
        })
    }
}

/// Поле занимает одну строку: пустое — не адрес, с переводом строки — вторая строка
/// запроса, то есть дописанный заголовок.
fn check_line(field: &'static str, value: &str) -> Result<(), SecretError> {
    if value.trim().is_empty() {
        return Err(SecretError::EmptyRequestField { field });
    }
    if value.contains(['\r', '\n']) {
        return Err(SecretError::ControlByte { field });
    }
    Ok(())
}

/// Прокси — единственное место, где токен существует. Он держит значение, подставляет его в
/// заголовок сам и отдаёт наружу тело; агент к значению не прикасается.
#[derive(Debug, Clone)]
pub struct TokenProxy {
    token: Token,
}

impl TokenProxy {
    /// Прокси создаётся из уже прочитанного токена: чтение файла — отдельный шаг со своей
    /// проверкой границы, а конструктор молча прочитал бы что попало и откуда угодно.
    pub fn new(token: Token) -> Self {
        Self { token }
    }

    /// Запрос уходит по сети текстом целиком: заголовок добавляет прокси, и существует он
    /// ровно в этой строке — в коде прокси. Транспорт тоже наш код: агент в сокет не пишет и
    /// из него не читает, поэтому и подставить токен в его аргументы некуда.
    ///
    /// Проверенное `ProxyRequest` не может выразить заголовок авторизации: поля для него нет,
    /// а переводы строк в остальных полях отвергнуты конструктором. Запрет дешевле проверки,
    /// а проверялось бы то, что и так нельзя передать.
    pub fn send<F>(
        &self,
        request: ProxyRequest<'_>,
        transport: F,
    ) -> Result<ProxyResponse, ProxyError>
    where
        F: FnOnce(&str) -> Result<(u16, String), String>,
    {
        // Значение подставляется из приватного поля: у `Token` нет даже `Display`, поэтому
        // и случайный `format!("{token}")` в отладочном коде секрет не напечатает.
        let mut wire = format!(
            "{} {} HTTP/1.1\r\nhost: {}\r\n",
            request.method, request.path, request.host
        );
        if let (Some(content_type), Some(body)) = (request.content_type, request.body) {
            wire.push_str(&format!(
                "content-type: {content_type}\r\ncontent-length: {}\r\n",
                body.len()
            ));
        }
        wire.push_str(&format!(
            "authorization: Bearer {}\r\n\r\n",
            self.token.value
        ));
        if let Some(body) = request.body {
            wire.push_str(body);
        }
        match transport(&wire) {
            // Тело проходит ту же смывку, что и причина отказа: внешний сервис вправе
            // отразить заголовок в ответе, а «вряд ли» в журнале неотличимо от «было».
            Ok((status, body)) => Ok(ProxyResponse {
                status,
                body: self.token.redact(&body),
            }),
            // Требование 4: причина может содержать эхо заголовка, поэтому смывается и она —
            // иначе отказ стал бы тем самым каналом утечки, который запрещён.
            Err(why) => Err(ProxyError::Upstream {
                reason: self.token.redact(&why),
            }),
        }
    }
}

/// Отказ прокси. Причина прошла смывку ДО того, как стала полем: `#[error]` печатает
/// `{reason}`, и любое другое место, где этот текст всплывёт, уже чистое.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProxyError {
    /// Транспорт отказал, сказав это. Сказав — с текстом, в котором токена уже нет.
    #[error("upstream refused the request: {reason}")]
    Upstream { reason: String },
}

/// Конфигурация прокси читается из файла ВНЕ песочницы агента. Под bind-назначением она была
/// бы файлом песочницы, а не файлом прокси: `bwrap` монтирует рабочую копию целиком и создаёт
/// недостающие каталоги в записываемом слое, поэтому «спрятать под корнем» нельзя — можно
/// только не дать пути оказаться под корнем.
pub fn read_token_outside(
    path: &Path,
    visible: &[PathBuf],
    name: &str,
) -> Result<Token, SecretError> {
    let asked = lexical(path);
    for root in visible {
        // Тот же смысл, что у `Sandbox::preflight`, и та же нормализация: путь под корнем
        // виден агенту, значит конфигурация прокси под ним быть не может.
        let root = lexical(root);
        if asked.starts_with(&root) {
            return Err(SecretError::ConfigInsideSandbox { root, path: asked });
        }
    }
    let why = |why: std::io::Error| SecretError::Unreadable {
        path: asked.clone(),
        why: why.to_string(),
    };
    let text = fs::read_to_string(&asked).map_err(why)?;
    Token::new(name, text.trim())
}

/// Нормализация лексическая: `..` и `.` решаются руками, без ФС — решать «по существованию»
/// нельзя, тогда граница врала бы ровно там, где опасно. Вверх за `/` не поднимаемся: нечего
/// отдавать. Это ТРЕТЬЕ повторение шести строк арифметики после `sandbox_bwrap::normalize` и
/// `session_identity::lexical`: обе приватны, а `files_allowed` пакета не даёт их трогать.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
