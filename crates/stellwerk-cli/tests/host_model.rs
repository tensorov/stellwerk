//! N2-00h1: `HostModel` — адрес, токен и сокет в одном месте.
//!
//! Тесты держат два свойства, ради которых точка входа существует: отсутствующая
//! настройка это отдельное состояние, а не дефолт, и токен не покидает прокси — ни в
//! `Debug`, ни в отказе. Последний тест идёт всей цепочкой: локальный HTTP-сервер,
//! реальный `model.sock`, настоящий разбор ответа.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream as NetStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stellwerk_cli::host_model::{HostModel, HostModelError, ModelConfig};
use stellwerk_task::m5::model_transport::Served;
use stellwerk_task::m5::secrets::SecretError;

const TOKEN_VALUE: &str = "sk-neuraldeep-canary-0001";

/// Каталог на тест: имя включает pid и метку, чтобы прогоны не делили сокет и токен.
fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("время")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "stellwerk-host-model-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("каталог теста");
    dir
}

fn write_token(dir: &Path) -> PathBuf {
    let path = dir.join("neuraldeep.token");
    std::fs::write(&path, format!("{TOKEN_VALUE}\n")).expect("файл токена");
    path
}

/// Один ход через сокет прокси: заявка `turn`, ответ — все кадры до `done`.
fn exchange(socket: &Path, text: &str) -> Vec<serde_json::Value> {
    let stream = UnixStream::connect(socket).expect("соединение с прокси");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("потолок чтения");
    let mut stream = stream;
    let wire = format!(
        "{}\n",
        serde_json::json!({
            "kind": "turn",
            "tools": [],
            "messages": [{ "role": "user", "text": text }],
        })
    );
    stream.write_all(wire.as_bytes()).expect("заявка записана");
    stream.flush().expect("заявка отправлена");
    let mut frames: Vec<serde_json::Value> = Vec::new();
    for line in BufReader::new(stream).lines() {
        let line = line.expect("строка кадра");
        frames.push(serde_json::from_str(&line).expect("кадр — json"));
        if frames
            .last()
            .and_then(|frame| frame.get("kind"))
            .and_then(serde_json::Value::as_str)
            == Some("done")
        {
            break;
        }
    }
    frames
}

/// Сервер отвечает одним телом: проверяется провод, а не сервис.
fn serve_json(body: &str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("локальный порт");
    let port = listener.local_addr().expect("адрес").port();
    let body = body.to_owned();
    let worker = std::thread::spawn(move || {
        let mut stream = accept_within(&listener);
        let mut request = String::new();
        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).expect("запрос прочитан");
        request.push_str(&String::from_utf8_lossy(&chunk[..read]));
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("ответ записан");
        stream.flush().expect("ответ отправлен");
        request
    });
    (format!("http://127.0.0.1:{port}/v1"), worker)
}

fn accept_within(listener: &TcpListener) -> NetStream {
    listener.set_nonblocking(true).expect("неблокирующий");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "ход не начался за 5 с");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(why) => panic!("соединение: {why}"),
        }
    }
}

/// П. 1: пустая настройка — отказ с именем поля. Ни один адрес не подставляется молча:
/// иначе первый живой ход ушёл бы «куда-нибудь» после утверждения плана человеком.
#[test]
fn a_missing_setting_is_an_explicit_unconfigured_state_not_a_default() {
    let dir = scratch("unconfigured");
    let model = ModelConfig::with_token_path("https://api.neuraldeep.ru/v1", "", write_token(&dir));
    match model {
        Err(HostModelError::Unconfigured { field }) => assert_eq!(field, "model_id"),
        other => panic!("ожидался отказ по model_id, получено {other:?}"),
    }
    let blank = ModelConfig::with_token_path("", "qwen3.6-unlim-xl-noreason", write_token(&dir));
    match blank {
        Err(HostModelError::Unconfigured { field }) => assert_eq!(field, "base_url"),
        other => panic!("ожидался отказ по base_url, получено {other:?}"),
    }
}

/// П. 2: файл токена под видимым корнем — отказ до чтения, и значения в тексте нет.
#[test]
fn a_token_file_inside_a_visible_root_is_refused() {
    let dir = scratch("inside");
    let token = write_token(&dir);
    let config = ModelConfig::with_token_path(
        "https://api.neuraldeep.ru/v1",
        "qwen3.6-unlim-xl-noreason",
        token,
    )
    .expect("настройка полна");
    let refused = HostModel::prepare(config, &dir, std::slice::from_ref(&dir))
        .expect_err("токен под корнем песочницы — отказ");
    assert!(
        matches!(
            refused,
            HostModelError::Secret(SecretError::ConfigInsideSandbox { .. })
        ),
        "{refused:?}"
    );
    assert!(!format!("{refused:?}").contains(TOKEN_VALUE));
}

/// П. 3: `Debug` собранной модели не печатает токен. Проверяется именно собранный объект:
/// производный `Debug` у вложенных типов — единственное место, где значение могло всплыть.
#[test]
fn the_debug_of_a_prepared_model_carries_no_token() {
    let dir = scratch("debug");
    let token = write_token(&dir);
    let config = ModelConfig::with_token_path(
        "https://api.neuraldeep.ru/v1",
        "qwen3.6-unlim-xl-noreason",
        token,
    )
    .expect("настройка полна");
    let model = HostModel::prepare(config, &dir, &[]).expect("модель собрана");
    let printed = format!("{model:?}");
    assert!(!printed.contains(TOKEN_VALUE), "токен в Debug: {printed}");
}

/// П. 4: сокет лежит в рабочем каталоге и исчезает вместе с моделью. Оставленный сокет
/// пережил бы агента и путал бы следующий запуск.
#[test]
fn the_socket_lives_in_the_scratch_directory_and_disappears_on_drop() {
    let dir = scratch("socket");
    let token = write_token(&dir);
    let config = ModelConfig::with_token_path(
        "https://api.neuraldeep.ru/v1",
        "qwen3.6-unlim-xl-noreason",
        token,
    )
    .expect("настройка полна");
    let model = HostModel::prepare(config, &dir, &[]).expect("модель собрана");
    let socket = model.socket().to_path_buf();
    assert_eq!(socket, dir.join("model.sock"));
    assert!(socket.exists(), "сокет создан при сборке");
    drop(model);
    assert!(!socket.exists(), "сокет удалён вместе с моделью");
}

/// П. 5: вся цепочка целиком. Заявка приходит в `model.sock`, уходит на провод как
/// chat/completions, ответ разбирается в кадр `text`, `done` добавляет прокси.
#[test]
fn one_turn_goes_through_the_proxy_and_the_proxy_adds_done() {
    let dir = scratch("turn");
    let token = write_token(&dir);
    let (base, worker) = serve_json(r#"{"choices":[{"message":{"content":"план готов"}}]}"#);
    let config = ModelConfig::with_token_path(&base, "qwen3.6-unlim-xl-noreason", token)
        .expect("настройка полна");
    let model = HostModel::prepare(config, &dir, &[]).expect("модель собрана");

    // Прокси обслуживает ровно одно соединение, поэтому ход запускается навсегда
    // отдельным потоком: иначе `exchange` ждал бы `accept`, которого никто не сделал.
    let frames = std::thread::scope(|scope| {
        let served = scope.spawn(|| model.serve_once().expect("ход обслужен"));
        let frames = exchange(model.socket(), "составь план");
        assert_eq!(served.join().expect("поток"), Served::Turn);
        frames
    });
    let request = worker.join().expect("сервер завершился");

    assert!(
        request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(request.contains("authorization: Bearer "), "{request}");
    assert!(
        request.contains(r#""model":"qwen3.6-unlim-xl-noreason""#),
        "в теле голый id модели: {request}"
    );
    assert_eq!(frames[0]["kind"], "text");
    assert_eq!(frames[0]["text"], "план готов");
    assert_eq!(frames[1]["kind"], "done");
    assert_eq!(frames[1]["reason"], "stop");
}
