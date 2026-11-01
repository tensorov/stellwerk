//! # ExecutionSpec: чем запускается агент, и отказ, пока это не названо
//!
//! `TaskSpec` отвечает на вопрос «что сделать». Вопрос «чем выполнить» — про запуск, и
//! `store`, который хранит `TaskSpec`, знать о запуске не должен (ADR-0015, решение 1).
//! Смешать их значило бы протащить программу агента в контракт задачи, а он ещё и без
//! `Default`: новое обязательное поле сломало бы пятнадцать чужих литералов `TaskSpec`.
//!
//! Закон 6 здесь — не украшение. «`ExecutionSpec` нет», «`ExecutionSpec` есть, но `program`
//! пуст» и «есть, и движок разрешён» — три РАЗНЫХ состояния. У каждого свой отказ, и ни
//! одно не превращается в другое: `Default`, `#[serde(default)]` и «нет ответа — значит
//! запустим что настроено» запрещены. Отказ возвращается ДО запуска: ни процесса, ни
//! песочницы при нём не существует, и этой функции нечем их создать.
//!
//! RED N1-02: тело `resolve` — `unimplemented!()`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Чем выполнить задачу: программа и движок. Оба поля обязательны.
///
/// `Default` нет и `#[serde(default)]` нет намеренно: пропущенное поле — это отсутствие
/// данных, и оно обязано стать ошибкой разбора, а не пустой строкой, неотличимой от
/// осознанного `program = ""`. Пустая строка на входе останавливается `resolve`, а не
/// молчаливой подстановкой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSpec {
    /// Путь или имя исполняемого файла.
    pub program: String,
    /// Идентификатор движка: по нему выбирается профиль из реестра.
    pub engine: String,
}

/// Разрешённый engine-профиль. Профиль называет движок и ЕГО аргументы запуска; программа
/// приходит из `ExecutionSpec` отдельно, чтобы «что запускать» называлось ровно одним
/// источником, а не двумя, расходящимися молча.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineProfile {
    engine: &'static str,
    args: &'static [&'static str],
}

impl EngineProfile {
    /// Профиль из идентификатора движка и его аргументов.
    pub const fn new(engine: &'static str, args: &'static [&'static str]) -> Self {
        Self { engine, args }
    }

    /// Идентификатор движка — ключ реестра и то, что уходит в evidence.
    pub fn engine(&self) -> &'static str {
        self.engine
    }

    /// Аргументы, с которыми профиль запускает движок. Пустой список — это «без
    /// аргументов», а не «аргументы не заданы»: список задан здесь же, в коде.
    pub fn args(&self) -> &'static [&'static str] {
        self.args
    }
}

/// Реестр разрешённых профилей. Живёт в КОДЕ, а не в конфиге репозитория. Пустой реестр
/// валиден и отказывает всему: «не разрешено ничего» отличимо от «разрешено всё», иначе
/// реестр не был бы реестром.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineRegistry {
    profiles: &'static [EngineProfile],
}

impl EngineRegistry {
    /// Реестр из явного перечня. Другого конструктора нет: реестр, собранный из окружения,
    /// был бы разным в каждом терминале.
    pub const fn of(profiles: &'static [EngineProfile]) -> Self {
        Self { profiles }
    }

    /// Найти профиль по идентификатору движка. `None` значит «не разрешён» — именно это и
    /// есть отказ реестра, а не «взять что-нибудь подходящее».
    pub fn find(&self, engine: &str) -> Option<&EngineProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.engine == engine)
    }

    /// Перечень разрешённых движков — для отказа, который обязан назвать, что было можно.
    pub fn engines(&self) -> Vec<String> {
        self.profiles
            .iter()
            .map(|profile| profile.engine.to_owned())
            .collect()
    }
}

/// Профили, разрешённые в этой сборке. `pi` — единственный разрешённый профиль (запуск —
/// N2-00). Второй живой профиль сделал бы недоказуемым утверждение «исполнялся именно
/// авторизованный движок»: запуск перестал бы называть ровно один движок. Список задан
/// здесь, в коде, и живёт здесь как данные, а не как ветка в `resolve`.
pub const ALLOWED_PROFILES: &[EngineProfile] = &[EngineProfile::new("pi", &[])];

/// Реестр этой сборки.
pub const REGISTRY: EngineRegistry = EngineRegistry::of(ALLOWED_PROFILES);

/// Снимок запуска, который ложится в Run рядом со спецификацией. Владеющие поля: профиль —
/// ссылки в статический реестр, а запись в evidence обязана читаться без реестра.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunExecution {
    /// Программа запуска.
    pub program: String,
    /// Идентификатор разрешённого движка.
    pub engine: String,
    /// Аргументы профиля, снятые в момент разрешения.
    pub args: Vec<String>,
}

/// Отказ до запуска. Варианты не сливаются и не сворачиваются в строку: «конфигурации нет»,
/// «программа пуста» и «движок не разрешён» чинятся по-разному, и одно, выданное под видом
/// другого, — это и есть дефект, против которого написан закон 6.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionRefusal {
    /// `ExecutionSpec` отсутствует вовсе. Это НЕ «пустая программа».
    #[error("no execution spec is configured: nothing names a program to launch")]
    Absent,
    /// `ExecutionSpec` есть, но `program` пуст. Это НЕ «движка нет» и НЕ «нет спецификации».
    #[error("execution spec names no program: `program` is blank, so there is nothing to launch")]
    EmptyProgram,
    /// Движок не значится в реестре. Имя названо; разрешения по умолчанию не существует.
    #[error("engine `{engine}` is not allowed: registry holds {known:?}")]
    UnknownProfile {
        /// Имя отвергнутого движка.
        engine: String,
        /// Что вообще было разрешено в этом реестре.
        known: Vec<String>,
    },
}

/// Выбранный профиль вместе с его спецификацией — то, что сохраняется в Run. Профиль
/// захвачен ЗДЕСЬ, в момент разрешения: чтение не пересчитывает его по реестру, иначе
/// evidence показывал бы сегодняшний реестр, а не тот профиль, которым запускали.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedExecution {
    spec: ExecutionSpec,
    profile: EngineProfile,
}

impl ResolvedExecution {
    /// Спецификация, которую разрешили.
    pub fn spec(&self) -> &ExecutionSpec {
        &self.spec
    }

    /// Профиль, которым будут запускать. Захвачен при разрешении, а не вычислен при чтении.
    pub fn profile(&self) -> &EngineProfile {
        &self.profile
    }

    /// Программа запуска — единственный источник: поле `program` спецификации.
    pub fn program(&self) -> &str {
        &self.spec.program
    }

    /// Идентификатор движка разрешённого профиля.
    pub fn engine(&self) -> &'static str {
        self.profile.engine()
    }

    /// Снимок для записи в Run: владеющие строки, чтобы запись не зависела от реестра в
    /// момент чтения. Собран из уже разрешённых значений, а не из повторного поиска.
    pub fn run_entry(&self) -> RunExecution {
        RunExecution {
            program: self.spec.program.clone(),
            engine: self.profile.engine().to_owned(),
            args: self
                .profile
                .args()
                .iter()
                .map(|arg| (*arg).to_owned())
                .collect(),
        }
    }
}

/// Разрешить запуск: выбрать профиль по спецификации — или отказать ДО запуска. Порядок
/// проверок идёт от «данных нет» к «данные неверны»: сначала отсутствие спецификации, потом
/// пустая программа, потом неизвестный движок. Каждое состояние останавливается своим
/// вариантом, и до `run_once` и создания песочницы дело не доходит — этой функции нечем
/// запускать процесс, у неё нет ни `Command`, ни песочницы.
pub fn resolve(
    spec: Option<&ExecutionSpec>,
    registry: &EngineRegistry,
) -> Result<ResolvedExecution, ExecutionRefusal> {
    // Отсутствие данных — не «пустая строка» и не «движок по умолчанию»: свой вариант.
    let spec = spec.ok_or(ExecutionRefusal::Absent)?;
    if spec.program.trim().is_empty() {
        return Err(ExecutionRefusal::EmptyProgram);
    }
    // Реестр решает допуск: движка в нём нет — отказать, а не «взять что-нибудь».
    let profile = registry
        .find(&spec.engine)
        .ok_or_else(|| ExecutionRefusal::UnknownProfile {
            engine: spec.engine.clone(),
            known: registry.engines(),
        })?;
    Ok(ResolvedExecution {
        spec: spec.clone(),
        profile: *profile,
    })
}
