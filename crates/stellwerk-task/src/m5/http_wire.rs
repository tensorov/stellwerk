//! # Провод: разбор базового адреса, TLS и чтение ответа
//!
//! Слой знает только байты: он получает готовый HTTP-запрос от `TokenProxy` и возвращает
//! код и тело. Токен здесь не читается и не печатается.
//!
//! `http` разрешён только на loopback: тесты не должны становиться способом отправить
//! токен открытым текстом. Всё остальное — `https` с обязательной проверкой сертификата.

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

const HTTPS_PORT: u16 = 443;
const HTTP_PORT: u16 = 80;
/// Потолок заголовков ответа: без него сервер, не закрывающий соединение, держал бы чтение
/// вечно, и «нет ответа» стало бы зависанием.
const MAX_HEAD: usize = 64 * 1024;

/// Разобранный базовый адрес: схема, хост, порт и префикс пути.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl {
    scheme: Scheme,
    host: String,
    port: u16,
    prefix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Https,
    LoopbackHttp,
}

impl BaseUrl {
    /// Схемы кроме `http`/`https` — отказ: «непонятная схема», угаданная как `https`,
    /// отправила бы запрос не туда.
    pub fn parse(base_url: &str) -> Result<Self, String> {
        let (scheme, rest) = base_url
            .split_once("://")
            .ok_or_else(|| "base url has no scheme".to_owned())?;
        let (scheme, default_port) = match scheme {
            "https" => (Scheme::Https, HTTPS_PORT),
            "http" => (Scheme::LoopbackHttp, HTTP_PORT),
            other => return Err(format!("unsupported scheme `{other}`")),
        };
        let (authority, prefix) = match rest.find('/') {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, ""),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (
                host,
                port.parse()
                    .map_err(|_| "port in base url is not a number".to_owned())?,
            ),
            None => (authority, default_port),
        };
        if host.trim().is_empty() {
            return Err("base url names no host".to_owned());
        }
        if scheme == Scheme::LoopbackHttp && !is_loopback(host) {
            return Err(
                "plain http is allowed only on loopback: the token would travel unencrypted"
                    .to_owned(),
            );
        }
        Ok(Self {
            scheme,
            host: host.to_owned(),
            port,
            prefix: prefix.trim_end_matches('/').to_owned(),
        })
    }

    /// Значение заголовка `host`: порт обязателен, потому что разбирать его из адреса
    /// сервису незачем, а расхождение с `connect` было бы молчаливым.
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Путь запроса: префикс из базового адреса плюс хвост метода.
    pub fn path(&self, tail: &str) -> String {
        format!("{}{tail}", self.prefix)
    }

    /// Один обмен: запись целиком, затем чтение ровно до конца тела по `content-length`.
    /// `read_to_end` зависел бы на закрытие соединения, а сервис вправе держать его открытым.
    pub fn exchange(&self, wire: &str) -> Result<(u16, String), String> {
        let stream = TcpStream::connect((self.host.as_str(), self.port))
            .map_err(|why| format!("connect: {why}"))?;
        match self.scheme {
            Scheme::LoopbackHttp => round_trip(stream, wire),
            Scheme::Https => {
                let name = ServerName::try_from(self.host.clone())
                    .map_err(|why| format!("server name: {why}"))?;
                let mut roots = RootCertStore::empty();
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                let config = ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth();
                let connection = ClientConnection::new(Arc::new(config), name)
                    .map_err(|why| format!("tls: {why}"))?;
                round_trip(StreamOwned::new(connection, stream), wire)
            }
        }
    }
}

fn round_trip<S: Read + Write>(mut stream: S, wire: &str) -> Result<(u16, String), String> {
    stream
        .write_all(wire.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|why| format!("send: {why}"))?;
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|why| format!("receive: {why}"))?;
        if read == 0 {
            return Err("service closed the connection before the body was complete".to_owned());
        }
        raw.extend_from_slice(&chunk[..read]);
        match find_head(&raw) {
            Some(head) => {
                if chunked(&raw[..head]) {
                    return Err("chunked response is not supported".to_owned());
                }
                let length = content_length(&raw[..head])
                    .ok_or_else(|| "response carries no content-length".to_owned())?;
                if raw.len() >= head + length {
                    let status = status(&raw[..head])?;
                    let body = String::from_utf8_lossy(&raw[head..head + length]).into_owned();
                    return Ok((status, body));
                }
            }
            None if raw.len() > MAX_HEAD => {
                return Err("response headers exceed the limit".to_owned());
            }
            None => {}
        }
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

/// Конец заголовков: пустая строка. Возвращается позиция первого байта тела.
fn find_head(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| at + 4)
}

fn head_line<'a>(head: &'a [u8], prefix: &str) -> Option<&'a str> {
    std::str::from_utf8(head).ok()?.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix(prefix)
            .map(|_| line[prefix.len()..].trim())
    })
}

fn content_length(head: &[u8]) -> Option<usize> {
    head_line(head, "content-length:")?.parse().ok()
}

fn chunked(head: &[u8]) -> bool {
    head_line(head, "transfer-encoding:")
        .map(|value| value.eq_ignore_ascii_case("chunked"))
        .unwrap_or(false)
}

fn status(head: &[u8]) -> Result<u16, String> {
    std::str::from_utf8(head)
        .ok()
        .and_then(|text| text.lines().next())
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| "response carries no status line".to_owned())
}
