//! V1/c02: грязный workspace не превращается в принятый кандидат.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use stellwerk_cli::build_runner::{
    accept_candidate, snapshot_workspace, CandidateFile, CandidateVerdict, DirtyWorkspace,
    HARNESS_EXTENSION,
};
use stellwerk_gateway::gate::ContentDigest;

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(name: &str) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("sw-c02-{name}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("каталог пробы");
    root
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("родитель файла");
    }
    std::fs::write(path, bytes).expect("файл пробы");
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("копия дерева");
    for entry in std::fs::read_dir(from).expect("чтение дерева") {
        let entry = entry.expect("запись дерева");
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let kind = std::fs::symlink_metadata(&src)
            .expect("метаданные")
            .file_type();
        if kind.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("копия файла");
        }
    }
}

fn dirty_files(verdict: CandidateVerdict) -> Vec<String> {
    match verdict {
        CandidateVerdict::Dirty(DirtyWorkspace { files }) => files,
        CandidateVerdict::Clean => panic!("ожидался отказ, получено Clean"),
    }
}

/// Лишний файл в workspace — это чужая правка, и кандидат обязан быть отклонён с
/// перечнем, а не принят молча.
#[test]
fn dirty_workspace_is_refused_with_file_list() {
    let root = scratch("dirty");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    copy_tree(&base, &work);
    write(&work, "src/extra.rs", b"fn extra() {}\n");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(dirty_files(verdict), vec!["src/extra.rs".to_string()]);

    let _ = std::fs::remove_dir_all(&root);
}

/// Дерево, совпадающее с базой, — отдельное состояние `Clean`, а не «нет отказа».
#[test]
fn clean_workspace_is_accepted() {
    let root = scratch("clean");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    write(&base, "README.md", b"# demo\n");
    copy_tree(&base, &work);

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(verdict, CandidateVerdict::Clean);

    let _ = std::fs::remove_dir_all(&root);
}

/// Кандидат перекрывает базу: если работающий файл совпадает с записью кандидата,
/// правка авторизована и дерево чистое.
#[test]
fn candidate_override_is_accepted() {
    let root = scratch("override");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    copy_tree(&base, &work);
    write(&work, "src/lib.rs", b"pub fn one() -> u8 { 2 }\n");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let candidate = vec![CandidateFile {
        path: "src/lib.rs".to_string(),
        digest: ContentDigest::of(b"pub fn one() -> u8 { 2 }\n")
            .as_hex()
            .to_string(),
    }];
    let verdict = accept_candidate(&work, &snapshot, &candidate).expect("проверка кандидата");

    assert_eq!(verdict, CandidateVerdict::Clean);

    let _ = std::fs::remove_dir_all(&root);
}

/// Несовпадение digest — это не «изменилось», а отказ: байты не те, что заявлены.
#[test]
fn digest_mismatch_is_refused() {
    let root = scratch("digest");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    copy_tree(&base, &work);
    write(&work, "src/lib.rs", b"pub fn one() -> u8 { 9 }\n");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(dirty_files(verdict), vec!["src/lib.rs".to_string()]);

    let _ = std::fs::remove_dir_all(&root);
}

/// Удалённый файл базы не представим парой «путь + digest», поэтому он обязан стать
/// отказом с именем, а не тихо исчезнуть из кандидата.
#[test]
fn deleted_base_file_is_refused() {
    let root = scratch("deleted");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    copy_tree(&base, &work);
    std::fs::remove_file(work.join("src/lib.rs")).expect("удаление файла пробы");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(dirty_files(verdict), vec!["src/lib.rs".to_string()]);

    let _ = std::fs::remove_dir_all(&root);
}

/// Смена цели симлинка — это правка, а не «ничего не произошло»: снимок хранит digest
/// текста ссылки, поэтому `link -> target-b.txt` отличается от `link -> target-a.txt`.
#[test]
fn symlink_target_change_is_dirty() {
    let root = scratch("symlink");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "target-a.txt", b"a\n");
    write(&base, "target-b.txt", b"b\n");
    std::os::unix::fs::symlink("target-a.txt", base.join("link")).expect("ссылка базы");
    copy_tree(&base, &work);
    std::fs::remove_file(work.join("link")).expect("удаление копии ссылки");
    std::os::unix::fs::symlink("target-b.txt", work.join("link")).expect("ссылка work");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(dirty_files(verdict), vec!["link".to_string()]);

    let _ = std::fs::remove_dir_all(&root);
}

/// `.git` и служебный файл харнесса — не правки агента и не должны давать грязь.
#[test]
fn git_and_harness_files_are_not_false_dirt() {
    let root = scratch("harness");
    let base = root.join("base");
    let work = root.join("work");
    write(&base, "src/lib.rs", b"pub fn one() -> u8 { 1 }\n");
    write(&base, HARNESS_EXTENSION, b"// extension\n");
    write(&base, ".git/HEAD", b"ref: refs/heads/main\n");
    copy_tree(&base, &work);
    write(&work, ".git/HEAD", b"ref: refs/heads/other\n");
    write(&work, ".git/index", b"binary index\n");

    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let verdict = accept_candidate(&work, &snapshot, &[]).expect("проверка кандидата");

    assert_eq!(verdict, CandidateVerdict::Clean);

    let _ = std::fs::remove_dir_all(&root);
}

/// Путь кандидата вне workspace или служебный путь не «пропускается», а становится
/// отказом с именем: абсолютный, `..`, `.git` и файл харнесса.
#[test]
fn candidate_paths_outside_the_workspace_are_refused() {
    let root = scratch("paths");
    let base = root.join("base");
    let work = root.join("work");
    std::fs::create_dir_all(&base).expect("каталог базы");
    std::fs::create_dir_all(&work).expect("каталог work");
    let snapshot = snapshot_workspace(&base).expect("снимок базы");
    let digest = ContentDigest::of(b"x").as_hex().to_string();
    let candidate = [
        "/tmp/outside.rs",
        "src/../escape.rs",
        ".git/config",
        HARNESS_EXTENSION,
    ]
    .map(|path| CandidateFile {
        path: path.to_string(),
        digest: digest.clone(),
    })
    .to_vec();

    let verdict = accept_candidate(&work, &snapshot, &candidate).expect("проверка кандидата");

    assert_eq!(
        dirty_files(verdict),
        vec![
            ".git/config".to_string(),
            "/tmp/outside.rs".to_string(),
            "src/../escape.rs".to_string(),
            HARNESS_EXTENSION.to_string(),
        ]
    );

    let _ = std::fs::remove_dir_all(&root);
}
