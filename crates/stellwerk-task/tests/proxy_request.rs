//! N2-00g2: запрос прокси несёт тело, но токен по-прежнему подставляет только прокси.
//!
//! Проверяется форма провода целиком, а не отдельные подстроки: порядок заголовков и
//! `content-length` в байтах — часть контракта с транспортом, и «примерно такой» запрос
//! означает, что сервис получит обрезанное тело. Отказы проверяются там, где они
//! происходят, — до вызова транспорта: отказ после отправки уже ничего не отменяет.

use stellwerk_task::m5::secrets::{ProxyRequest, SecretError, Token, TokenProxy, MAX_BODY};

const NAME: &str = "NEURALDEEP_TOKEN";
const VALUE: &str = "nd-t0ken-never-on-the-wire-twice";
const HOST: &str = "api.example.ru";
const PATH: &str = "/v1/chat/completions";

fn proxy() -> TokenProxy {
    TokenProxy::new(Token::new(NAME, VALUE).expect("имя и значение заданы"))
}

/// П. 1: тело и его заголовки доходят до транспорта ровно в том виде, в каком их ждёт
/// сервис. Сравнение — всей строки: подстрочный поиск пропустил бы и переставленный
/// `content-length`, и забытый `host`.
#[test]
fn the_wire_carries_host_content_type_and_the_body_byte_for_byte() {
    let body = "{\"model\":\"m\",\"messages\":[{\"role\":\"user\",\"content\":\"привет\"}]}";
    let request = ProxyRequest::new("POST", HOST, PATH, Some("application/json"), Some(body))
        .expect("корректный запрос");
    let sent = proxy().send(request, |wire| {
        let expected = format!(
            "POST {PATH} HTTP/1.1\r\nhost: {HOST}\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nauthorization: Bearer {VALUE}\r\n\r\n{body}",
            body.len()
        );
        assert_eq!(wire, expected);
        Ok((200, "{}".to_owned()))
    });
    assert_eq!(sent.expect("прокси отвечает").status, 200);
}

/// П. 2: тело без типа — не «пусть сервис догадается», а отказ: разбор чужого тела по
/// догадке вернул бы «непонятый запрос» вместо «мы его неверно собрали».
#[test]
fn a_body_without_a_content_type_is_refused_before_the_transport_runs() {
    let refused =
        ProxyRequest::new("POST", HOST, PATH, None, Some("{}")).expect_err("тело без типа — отказ");
    assert!(matches!(refused, SecretError::BodyWithoutContentType));
}

/// П. 3: потолок тела проверяется ДО транспорта. Иначе отказ означал бы «мы уже отправили
/// мегабайт, а потом передумали».
#[test]
fn an_oversize_body_is_refused_before_the_transport_runs() {
    let big = "x".repeat(MAX_BODY + 1);
    let refused = ProxyRequest::new("POST", HOST, PATH, Some("application/json"), Some(&big))
        .expect_err("тело сверх потолка — отказ");
    assert!(matches!(
        refused,
        SecretError::BodyTooLarge { len, max } if len == MAX_BODY + 1 && max == MAX_BODY
    ));
}

/// П. 4: заголовок авторизации вставляет прокси — ровно один раз. Задание запроса не
/// содержит поля для него, поэтому «второй заголовок» не может прийти от вызывающего.
#[test]
fn the_authorization_header_appears_exactly_once() {
    let request =
        ProxyRequest::new("GET", HOST, "/user", None, None).expect("запрос без тела разрешён");
    let mut seen = String::new();
    proxy()
        .send(request, |wire| {
            seen.push_str(wire);
            Ok((200, "{}".to_owned()))
        })
        .expect("прокси отвечает");
    assert_eq!(seen.to_lowercase().matches("authorization:").count(), 1);
    assert_eq!(seen.matches(VALUE).count(), 1);
}

/// П. 5: перевод строки в пути — попытка дописать заголовок, а не часть адреса. Отказ
/// называет поле: «неверный запрос» без имени чинится чтением исходника.
#[test]
fn a_control_character_in_a_request_field_is_refused_by_name() {
    let injected = ProxyRequest::new("GET", HOST, "/user\r\nx-evil: 1", None, None);
    assert!(matches!(
        injected,
        Err(SecretError::ControlByte { field: "path" })
    ));
    let host = ProxyRequest::new("GET", "api.example.ru\n", "/user", None, None);
    assert!(matches!(
        host,
        Err(SecretError::ControlByte { field: "host" })
    ));
    let relative = ProxyRequest::new("GET", HOST, "v1/chat", None, None);
    assert!(matches!(relative, Err(SecretError::PathNotAbsolute)));
}
