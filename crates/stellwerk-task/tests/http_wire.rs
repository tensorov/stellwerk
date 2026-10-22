//! N2-00g3a: провод — адрес, соединение и чтение ответа по байтам.
//!
//! Сервер здесь настоящий, но локальный: TLS в этих тестах не участвует, зато проверяется
//! правило, ради которого он нужен, — открытый HTTP вне loopback отвергается до соединения.
//! Отдельно проверяется, что «нет длины тела» и «chunked» — отказы, а не пустое тело.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use stellwerk_task::m5::http_wire::BaseUrl;

/// Соединение принимается с потолком: обмен, который не состоялся, обязан провалить тест,
/// а не держать поток `accept` до таймаута всего прогона.
fn accept_within(listener: &TcpListener) -> std::net::TcpStream {
    listener.set_nonblocking(true).expect("неблокирующий слушатель");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "обмен не начался за 5 с");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(why) => panic!("соединение: {why}"),
        }
    }
}

/// Сервер отдаёт ровно те байты, что дал тест: иначе «нет content-length» проверялось бы
/// на ответе, который его содержит.
fn serve_raw(response: String) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("локальный порт");
    let port = listener.local_addr().expect("адрес").port();
    let worker = std::thread::spawn(move || {
        let mut stream = accept_within(&listener);
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).expect("запрос прочитан");
        request.extend_from_slice(&chunk[..read]);
        stream
            .write_all(response.as_bytes())
            .expect("ответ записан");
        stream.flush().expect("ответ отправлен");
        String::from_utf8_lossy(&request).into_owned()
    });
    (format!("http://127.0.0.1:{port}/v1"), worker)
}

/// П. 1: адрес разбирается целиком — порт по умолчанию для схемы и префикс пути, без
/// которого запрос ушёл бы в корень сервиса.
#[test]
fn the_base_url_yields_authority_and_path_with_the_prefix() {
    let base = BaseUrl::parse("https://api.example.ru/v1/").expect("адрес разобран");
    assert_eq!(base.authority(), "api.example.ru:443");
    assert_eq!(base.path("/chat/completions"), "/v1/chat/completions");

    let explicit = BaseUrl::parse("http://127.0.0.1:8080").expect("адрес разобран");
    assert_eq!(explicit.authority(), "127.0.0.1:8080");
    assert_eq!(explicit.path("/x"), "/x");
}

/// П. 2: открытый HTTP вне loopback — отказ до соединения, и причина это называет.
#[test]
fn plain_http_outside_loopback_is_refused() {
    let refused = BaseUrl::parse("http://api.example.ru/v1").expect_err("не loopback");
    assert!(refused.contains("loopback"), "{refused}");
    let unknown = BaseUrl::parse("ftp://api.example.ru/v1").expect_err("чужая схема");
    assert!(unknown.contains("scheme"), "{unknown}");
}

/// П. 3: обмен возвращает код и тело ровно той длины, что объявлена.
#[test]
fn an_exchange_returns_the_status_and_the_declared_body() {
    let (url, worker) = serve_raw(
        "HTTP/1.1 200 OK\r\ncontent-length: 7\r\nconnection: close\r\n\r\n{\"a\":1}".to_owned(),
    );
    let (status, body) = BaseUrl::parse(&url)
        .expect("адрес разобран")
        .exchange("GET /v1 HTTP/1.1\r\nhost: x\r\n\r\n")
        .expect("обмен состоялся");
    let request = worker.join().expect("сервер завершился");
    assert_eq!(status, 200);
    assert_eq!(body, "{\"a\":1}");
    assert!(request.starts_with("GET /v1 HTTP/1.1\r\n"), "{request}");
}

/// П. 4: `chunked` — отказ, а не обрезанное тело: собрать его без разбора чанков значило бы
/// отдать вызывающему половину ответа как целый.
#[test]
fn a_chunked_response_is_refused() {
    let (url, worker) = serve_raw(
        "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n7\r\n{\"a\":1}\r\n0\r\n\r\n"
            .to_owned(),
    );
    let refused = BaseUrl::parse(&url)
        .expect("адрес разобран")
        .exchange("GET /v1 HTTP/1.1\r\nhost: x\r\n\r\n")
        .expect_err("chunked — отказ");
    worker.join().expect("сервер завершился");
    assert!(refused.contains("chunked"), "{refused}");
}

/// П. 5: ответ без длины тела — отказ. Молчаливое «до закрытия соединения» вернуло бы
/// пустоту на keep-alive и выдало бы её за ответ.
#[test]
fn a_response_without_content_length_is_refused() {
    let (url, worker) = serve_raw("HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n{}".to_owned());
    let refused = BaseUrl::parse(&url)
        .expect("адрес разобран")
        .exchange("GET /v1 HTTP/1.1\r\nhost: x\r\n\r\n")
        .expect_err("нет длины — отказ");
    worker.join().expect("сервер завершился");
    assert!(refused.contains("content-length"), "{refused}");
}
