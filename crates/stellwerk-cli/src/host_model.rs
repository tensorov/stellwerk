//! # `HostModel`: честная точка входа к модели
//!
//! Здесь сходятся три внешние величины — базовый адрес, id модели и файл токена — и
//! получается пара «прокси + backend». Больше нигде в коде эти три не соединяются, поэтому
//! и «настройка не задана» существует ровно в одном месте.
//!
//! Отсутствие значения — отдельное состояние (`Unconfigured` с именем поля), а не дефолт:
//! придуманный адрес выглядел бы рабочей настройкой и упал бы уже после того, как человек
//! утвердил план.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use stellwerk_task::m5::model_backend::HttpBackend;
use stellwerk_task::m5::model_transport::{BackendError, ModelProxy, Served};
use stellwerk_task::m5::secrets::{read_token_outside, SecretError, TokenProxy};

/// Имя переменной окружения, под которым токен живёт в Pi. Оно же имя секрета в отказе.
const TOKEN_NAME: &str = "NEURALDEEP_TOKEN";

/// Что пошло не так при сборке точки входа.
#[derive(Debug)]
pub enum HostModelError {
    /// Значение не задано. Поле называется: «чего-то нет» и «нет именно адреса» — разные
    /// сообщения, иначе вызывающий не может ничего исправить.
    Unconfigured { field: &'static str },
    /// Токен не прочитан или лежит не там, где ему место.
    Secret(SecretError),
    /// Адрес или модель отвергнуты до первого хода.
    Backend(BackendError),
    /// Сокет прокси не создан.
    Socket { why: String },
}

impl fmt::Display for HostModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unconfigured { field } => write!(f, "model setting `{field}` is not configured"),
            Self::Secret(why) => write!(f, "token refused: {why}"),
            Self::Backend(why) => write!(f, "model refused: {why}"),
            Self::Socket { why } => write!(f, "proxy socket: {why}"),
        }
    }
}

impl std::error::Error for HostModelError {}

/// Три величины, без которых ход не начинается.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    base_url: String,
    model_id: String,
    token_path: PathBuf,
}

impl ModelConfig {
    /// Явный путь токена: вызывающий отвечает за него сам. Нужен и тестам, и тем, у кого
    /// файл лежит не под `XDG_CONFIG_HOME`.
    pub fn with_token_path(
        base_url: &str,
        model_id: &str,
        token_path: impl Into<PathBuf>,
    ) -> Result<Self, HostModelError> {
        if base_url.trim().is_empty() {
            return Err(HostModelError::Unconfigured { field: "base_url" });
        }
        if model_id.trim().is_empty() {
            return Err(HostModelError::Unconfigured { field: "model_id" });
        }
        Ok(Self {
            base_url: base_url.to_owned(),
            model_id: model_id.to_owned(),
            token_path: token_path.into(),
        })
    }

    /// Путь токена по правилу владельца: `$XDG_CONFIG_HOME/stellwerk/neuraldeep.token`,
    /// а при отсутствии `XDG_CONFIG_HOME` — `$HOME/.config/stellwerk/neuraldeep.token`.
    pub fn from_env(base_url: &str, model_id: &str) -> Result<Self, HostModelError> {
        let path = token_path(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
        .ok_or(HostModelError::Unconfigured {
            field: "token_path",
        })?;
        Self::with_token_path(base_url, model_id, path)
    }

    pub fn token_path(&self) -> &Path {
        &self.token_path
    }
}

fn token_path(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let root = match xdg {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(home?).join(".config"),
    };
    Some(root.join("stellwerk").join("neuraldeep.token"))
}

/// Собранная точка входа. Токен уже внутри прокси и наружу не выходит.
pub struct HostModel {
    proxy: ModelProxy,
    backend: HttpBackend,
    model_id: String,
}

impl HostModel {
    /// Порядок важен: токен читается до того, как появится сокет. Неудачный запуск не
    /// оставляет после себя слушателя, к которому кто-то успел бы подключиться.
    pub fn prepare(
        config: ModelConfig,
        scratch: &Path,
        visible: &[PathBuf],
    ) -> Result<Self, HostModelError> {
        let token = read_token_outside(&config.token_path, visible, TOKEN_NAME)
            .map_err(HostModelError::Secret)?;
        let backend = HttpBackend::new(&config.base_url, &config.model_id, TokenProxy::new(token))
            .map_err(HostModelError::Backend)?;
        let proxy = ModelProxy::bind(scratch).map_err(|why| HostModelError::Socket {
            why: why.to_string(),
        })?;
        Ok(Self {
            proxy,
            backend,
            model_id: config.model_id,
        })
    }

    pub fn socket(&self) -> &Path {
        self.proxy.socket()
    }

    /// Один ход. Защита от `Drop`-неожиданностей не нужна: прокси живёт ровно столько же,
    /// сколько модель.
    pub fn serve_once(&self) -> std::io::Result<Served> {
        self.proxy.serve_once(&self.backend)
    }

    /// Обслуживает канал модели, пока `stop` не поднят.
    ///
    /// `accept` не просыпается сам: остановка — это поднятый флаг **и** пустое
    /// подключение (`wake`). Подключение даёт `EOF` → `Served::NoRequest`, цикл видит
    /// флаг и выходит. Возвращает число обслуженных ходов; ноль — ходов не было, и это
    /// отличается от «были и все прошли».
    pub fn serve_until_stopped(&self, stop: &AtomicBool) -> std::io::Result<usize> {
        let mut served = 0;
        while !stop.load(Ordering::SeqCst) {
            if self.serve_once()? != Served::NoRequest {
                served += 1;
            }
        }
        Ok(served)
    }
}

impl fmt::Debug for HostModel {
    /// Ручной `Debug`, а не производный: производный прошёлся бы по `HttpBackend` и
    /// `TokenProxy`, и вопрос «печатается ли токен» решался бы в чужом файле.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostModel")
            .field("socket", &self.proxy.socket())
            .field("model_id", &self.model_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::token_path;

    /// Оба пути — под `stellwerk/neuraldeep.token`: имя файла не должно зависеть от того,
    /// задан ли `XDG_CONFIG_HOME`.
    #[test]
    fn the_token_path_follows_xdg_and_falls_back_to_home() {
        let xdg = token_path(Some("/xdg".into()), Some("/home/human".into())).expect("xdg задан");
        assert_eq!(xdg, std::path::Path::new("/xdg/stellwerk/neuraldeep.token"));

        let home = token_path(None, Some("/home/human".into())).expect("home задан");
        assert_eq!(
            home,
            std::path::Path::new("/home/human/.config/stellwerk/neuraldeep.token")
        );

        assert!(token_path(None, None).is_none(), "нечего подставлять");
    }
}
