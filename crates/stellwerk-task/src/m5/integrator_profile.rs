//! # Профиль интегратора: чужой секрет нечитаем, а не «не напечатан»
//!
//! Инцидент повторился дважды: управляющий агент прочитал глобальный конфиг с действующими
//! ключами и напечатал их в отчёт. Рабочий агент был ограничен песочницей, управляющий — нет,
//! и ограничение отсутствовало физически. Правило «не печатать секреты» защищает вывод, а
//! инцидент был в ЧТЕНИИ: путь к значению оставался, и печатающим мог оказаться любой.
//!
//! Здесь закрывается именно чтение, и тремя независимыми средствами. Первое — перечень видимых
//! путей, заданный В КОДЕ: профиль, собранный из окружения, был бы другим в каждом терминале,
//! и «агент секрета не видел» стало бы утверждением о конкретном запуске. Второе — сведения о
//! модели вида `{id, limits, caps}`, где поля, в которое поместился бы ключ, нет вовсе: не
//! «мы его не кладём», а «поля нет», и потому `Debug` выводится без оговорок. Третье — путь к
//! модели, которым остаётся `m5::secrets::TokenProxy`: он подставляет значение в заголовок сам
//! и отдаёт наружу тело ответа, а `Token` не имеет даже `Display`.
//!
//! Песочницу профиль НЕ строит: корни он отдаёт, а `bwrap` собирает вызывающий
//! (`m5::sandbox_bwrap` — конфигурация целиком его работа). Иначе окно песочницы и каталог
//! временных файлов уехали бы сюда и разошлись бы с тем, что агент запускают на самом деле.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::m5::session_identity::{IdentityError, ToolSet};

/// Системные каталоги, видимые интегратору на чтение. СПИСОК, а не «всё, кроме запрещённого»:
/// список запретов уезжает в переменную, добавленную позже, и профиль менялся бы от запуска к
/// запуску вместе с ней. Ровно столько, сколько нужно ЗАПУСТИТЬ программу: `/etc`, `$HOME`,
/// `/run` и `/var` сюда не входят, а именно там лежат глобальные конфиги, из-за которых этот
/// профиль и написан.
const SYSTEM_ROOTS: [&str; 3] = ["/usr", "/bin", "/lib64"];

/// Инструменты интегратора — исчерпывающий перечень, как `session_identity::ToolSet` у
/// субагента: нет в списке — недоступен. Инструмента, читающего чужой конфиг, здесь нет, и
/// «прочитать конфиг» не выражается словом «по умолчанию».
const INTEGRATOR_TOOLS: [&str; 4] = ["task_read", "task_write", "task_run", "task_report"];

/// Конфиги, к которым у интегратора не должно быть пути. ИМЕНА, а не содержимое: модуль их не
/// открывает и не печатает, а перечисление нужно, чтобы «профиль закрыт» проверялось
/// перечнем, а не отсутствием строки в коде.
pub const FOREIGN_CONFIGS: [&str; 3] = [
    "/etc/stellwerk/integrator.json",
    "/home/integrator/.config/agent/auth.json",
    "/run/stellwerk/proxy.env",
];

/// Отказ профиля. Варианты не сливаются: «путь вне профиля», «поле вне перечня», «поля нет»,
/// «поле повторено» и «строка не разобрана» чинятся по-разному, а молчаливое слияние отдало бы
/// «нет данных» под видом отказа (закон 6).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileError {
    /// Путь не под корнем профиля. Это граница ЧТЕНИЯ, а не вывода: дверь отсутствует, и
    /// потому «мы ничего не выводили» даже не понадобится.
    #[error(
        "`{path}` is outside the integrator profile {roots:?}: the integrator has no path to it"
    )]
    OutsideProfile { roots: Vec<PathBuf>, path: PathBuf },
    /// Имя поля каталога вне закрытого перечня сведений о модели. В отказе — только ИМЯ: сама
    /// строка содержала бы значение, и отказ стал бы тем каналом, который закрывают.
    #[error("catalog field `{field}` is not one of `id`, `limits`, `caps`: a credential has no field to land in")]
    UnlistedField { field: String },
    /// Поле повторено: два утверждения об одном имени, и «взяли последнее» стёрло бы различие
    /// между ними молча.
    #[error("catalog field `{field}` is stated twice: two claims, one name")]
    RepeatedField { field: &'static str },
    /// Обязательного поля нет. Отсутствие данных, а не пустое значение по умолчанию.
    #[error("catalog has no `{field}`: an absent field is not an empty value")]
    AbsentField { field: &'static str },
    /// Строка каталога не вида `поле=значение`. Строка не повторяется в отказе — по той же
    /// причине, что и значение поля.
    #[error("catalog entry is not `field=value`; the entry is never echoed")]
    Malformed,
    /// Набор инструментов интегратора не собирается. Список задан константой, поэтому это
    /// правка кода, а не настройка запуска, и отказ не должен молча превращаться в пустой набор.
    #[error("integrator tool set is unusable: {source}")]
    ToolSet {
        #[from]
        source: IdentityError,
    },
}

/// Профиль интегратора: рабочий каталог, системные корни и набор инструментов — три поля, и
/// все три известны до запуска. Ни путей, ни окружения конструктор не принимает, значит профиль
/// нельзя ни расширить, ни сузить, не правя код.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegratorProfile {
    work: PathBuf,
    system: Vec<PathBuf>,
    tools: ToolSet,
}

impl IntegratorProfile {
    /// Минимум: рабочий каталог — и всё. Ошибка набора пробрасывается, а не гасится: список
    /// задан константой модуля, и «профиль без инструментов» выглядел бы как успешная сборка.
    pub fn minimal(work: &Path) -> Result<Self, ProfileError> {
        Ok(Self {
            work: work.to_owned(),
            system: SYSTEM_ROOTS.iter().map(PathBuf::from).collect(),
            tools: ToolSet::listed(&INTEGRATOR_TOOLS)?,
        })
    }

    /// Рабочий каталог интегратора — единственный записываемый.
    pub fn work(&self) -> &Path {
        &self.work
    }

    /// Системные каталоги на чтение. Их кладёт в `Sandbox::read_only` вызывающий.
    pub fn system_roots(&self) -> &[PathBuf] {
        &self.system
    }

    /// Набор инструментов — исчерпывающий, как его и проверяет `session_identity`.
    pub fn tools(&self) -> &ToolSet {
        &self.tools
    }

    /// Всё, что интегратор видит: рабочий каталог и системные корни.
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut all = Vec::with_capacity(self.system.len() + 1);
        all.push(self.work.clone());
        all.extend(self.system.iter().cloned());
        all
    }

    /// Единственная дверь агента к файлу: путь вне профиля не разрешён. Проверка лексическая —
    /// `canonicalize` уронил бы ещё не существующий путь, а решить «по существованию» нельзя,
    /// тогда граница врала бы ровно там, где опасно. Вверх за `/` не поднимаемся: нечего отдавать.
    pub fn admits<'p>(&self, path: &'p Path) -> Result<&'p Path, ProfileError> {
        let asked = lexical(path);
        let roots = self.roots();
        if roots.iter().any(|root| asked.starts_with(root)) {
            Ok(path)
        } else {
            Err(ProfileError::OutsideProfile { roots, path: asked })
        }
    }
}

/// Сведения о модели — ровно `{id, limits, caps}`. Поля, в которое поместился бы ключ, в
/// структуре нет, поэтому секрету некуда деться: не «мы его не кладём», а «поля нет».
/// Значения несут названия ограничений и возможностей, а не их разбор, — разбор остаётся
/// вызывающему, иначе в поле пришлось бы класть то, что разбирается в типе.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFacts {
    /// Идентификатор модели как его знает каталог.
    pub id: String,
    /// Ограничения запроса.
    pub limits: Vec<String>,
    /// Заявленные возможности.
    pub caps: Vec<String>,
}

impl ModelFacts {
    /// Строка каталога вида `поле=значение;…`. Перечень полей ЗАКРЫТ, а не «известные читаем,
    /// остальное пропускаем»: пропуск и есть тот путь, по которому `api_key=` доезжает до
    /// агента в виде «мы его просто не разобрали».
    pub fn parse(line: &str) -> Result<Self, ProfileError> {
        let mut id: Option<String> = None;
        let mut limits: Vec<String> = Vec::new();
        let mut caps: Vec<String> = Vec::new();
        for entry in line.split(';').map(str::trim).filter(|e| !e.is_empty()) {
            let (field, value) = entry.split_once('=').ok_or(ProfileError::Malformed)?;
            let (field, value) = (field.trim(), value.trim());
            match field {
                "id" if id.is_none() => id = Some(value.to_owned()),
                "id" => return Err(ProfileError::RepeatedField { field: "id" }),
                "limits" => push_list(&mut limits, value),
                "caps" => push_list(&mut caps, value),
                other => {
                    return Err(ProfileError::UnlistedField {
                        field: other.to_owned(),
                    })
                }
            }
        }
        let id = id.ok_or(ProfileError::AbsentField { field: "id" })?;
        Ok(Self { id, limits, caps })
    }
}

/// Список значений через запятую. Пустая ячейка пропускается, а не становится пустым именем:
/// `caps=chat,` — это одно возможность, а не две, вторая из которых выглядит пустой.
fn push_list(into: &mut Vec<String>, value: &str) {
    into.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_owned),
    );
}

/// Нормализация лексическая: `..` и `.` разложены руками, без ФС. Это ЧЕТВЁРТОЕ повторение
/// шести строк арифметики после `sandbox_bwrap::normalize`, `session_identity::lexical` и
/// `secrets::lexical`: все три приватны, а `files_allowed` пакета не даёт их трогать.
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
