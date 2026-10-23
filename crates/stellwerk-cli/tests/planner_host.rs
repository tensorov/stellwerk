//! N2-00h2a: ход планировщика через наш транспорт.
//!
//! Проверяется ровно то, что видит TUI: текст плана, отказ сервиса, пустоту вместо плана —
//! и что два вопроса подряд на одном сокете работают (кнопка повторного ответа в TUI).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream as NetStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use stellwerk_cli::host_model::ModelConfig;
use stellwerk_cli::planner_host::{PlannerError, PlannerHost};

const TOKEN_VALUE: &str = "sk-neuraldeep-canary-0001";

fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("время")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "stellwerk-planner-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("каталог теста");
    std::fs::write(dir.join("neuraldeep.token"), format!("{TOKEN_VALUE}\n")).expect("файл токена");
    dir
}

/// Готовый планировщик и «сервис», который отдаёт ответы по очереди: так проверяется и
/// повторный ход, и отказ, не поднимая второй сервер.
fn planner(
    tag: &str,
    replies: Vec<(u16, String)>,
) -> (PlannerHost, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("локальный порт");
    let port = listener.local_addr().expect("адрес").port();
    let worker = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let mut stream = accept_within(&listener);
            let mut chunk = [0u8; 2048];
            let read = stream.read(&mut chunk).expect("запрос прочитан");
            requests.push(String::from_utf8_lossy(&chunk[..read]).into_owned());
            let reason = if status == 200 { "OK" } else { "ERR" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("ответ записан");
            stream.flush().expect("ответ отправлен");
        }
        requests
    });
    let dir = scratch(tag);
    let config = ModelConfig::with_token_path(
        &format!("http://127.0.0.1:{port}/v1"),
        "qwen3.6-unlim-xl-noreason",
        dir.join("neuraldeep.token"),
    )
    .expect("настройка полна");
    let model = PlannerHost::prepare(config, &dir, &[]).expect("планировщик собран");
    (model, worker)
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

fn choice(content: &str) -> String {
    serde_json::json!({ "choices": [{ "message": { "content": content } }] }).to_string()
}

/// П. 1: ход возвращает текст плана, а заявка ушла на провод как chat/completions.
#[test]
fn one_turn_returns_the_model_text() {
    let (planner, worker) = planner("text", vec![(200, choice("план готов"))]);
    let text = planner.ask("цель: добавить тест").expect("ход состоялся");
    let requests = worker.join().expect("сервис завершился");
    assert_eq!(text, "план готов");
    assert!(
        requests[0].starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
        "{}",
        requests[0]
    );
    assert!(
        requests[0].contains("цель: добавить тест"),
        "промпт дошёл до сервиса: {}",
        requests[0]
    );
}

/// П. 2: два хода на одном сокете. Прокси обслуживает одно соединение за вызов, и если
/// второй вопрос не поднимает его снова, кнопка повтора в TUI молча ничего не делает.
#[test]
fn two_turns_share_one_socket() {
    let (planner, worker) = planner(
        "twice",
        vec![(200, choice("первый")), (200, choice("второй"))],
    );
    assert_eq!(planner.ask("раз").expect("первый ход"), "первый");
    assert_eq!(planner.ask("два").expect("второй ход"), "второй");
    worker.join().expect("сервис завершился");
}

/// П. 3: код отказа сервиса — отказ, а не пустой план.
#[test]
fn a_failed_status_is_a_refusal_not_an_empty_plan() {
    let (planner, worker) = planner(
        "status",
        vec![(500, r#"{"error":"upstream exploded"}"#.to_owned())],
    );
    let refused = planner.ask("цель").expect_err("500 — отказ");
    worker.join().expect("сервис завершился");
    assert!(
        matches!(refused, PlannerError::Refused { .. }),
        "{refused:?}"
    );
    assert!(format!("{refused}").contains("500"), "код виден: {refused}");
}

/// П. 4: пустой `content` отвергает backend — ход без кадров неотличим от «сервис не
/// ответил», и планировщик получает отказ, а не пустой план на утверждение человеку.
#[test]
fn an_empty_message_is_refused_rather_than_becoming_an_empty_plan() {
    let (planner, worker) = planner("empty", vec![(200, choice(""))]);
    let refused = planner.ask("цель").expect_err("пустой ответ");
    worker.join().expect("сервис завершился");
    assert!(
        matches!(refused, PlannerError::Refused { .. }),
        "{refused:?}"
    );
    assert!(!format!("{refused}").contains("empty plan"));
}

/// П. 5: сервис вернул токен в теле ошибки — в отказе значения нет.
#[test]
fn the_token_never_appears_in_the_refusal() {
    let echo = format!(r#"{{"error":"invalid key {TOKEN_VALUE}"}}"#);
    let (planner, worker) = planner("token", vec![(401, echo)]);
    let refused = planner.ask("цель").expect_err("401 — отказ");
    let text = format!("{refused}");
    worker.join().expect("сервис завершился");
    assert!(text.contains("401"), "код виден: {text}");
    assert!(!text.contains(TOKEN_VALUE), "токен в отказе: {text}");
}

/// П. 7: инструменты планировщика не уходят сервису: в HTTP-теле нет ключа `tools`.
#[test]
fn the_planner_does_not_forward_tools_to_the_service() {
    let (planner, worker) = planner("notools", vec![(200, choice("план без инструментов"))]);
    let text = planner.ask("цель").expect("ход состоялся");
    let requests = worker.join().expect("сервис завершился");
    assert_eq!(text, "план без инструментов");
    assert!(
        !requests[0].contains("\"tools\""),
        "тело несёт tools: {}",
        requests[0]
    );
}

/// П. 6 (запас): отсутствующий сокет — отказ канала, а не паника и не пустой план.
#[test]
fn a_missing_socket_is_a_channel_error() {
    let dir = scratch("nosock");
    let config = ModelConfig::with_token_path(
        "http://127.0.0.1:1/v1",
        "qwen3.6-unlim-xl-noreason",
        dir.join("neuraldeep.token"),
    )
    .expect("настройка полна");
    let model = PlannerHost::prepare(config, &dir, &[]).expect("планировщик собран");
    std::fs::remove_file(model.socket()).expect("сокет убран");
    let refused = model.ask("цель").expect_err("нет сокета");
    assert!(matches!(refused, PlannerError::Channel(_)), "{refused:?}");
}
