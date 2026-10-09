use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// The rail shows at most this many folders; the oldest entry is evicted.
pub const MAX_ENTRIES: usize = 50;
const STORE_FILE: &str = "history.json";
const STORE_VERSION: u32 = 1;

static STORE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    STORE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// A `.enc` file sealed through the app; `restored` flips when the user proves
/// recovery by decrypting it again (the sealed copy always stays).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SealedFile {
    pub path: String,
    pub sealed_at: u64,
    #[serde(default)]
    pub restored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Display path exactly as the user picked it.
    pub root: String,
    /// Canonical, case-folded identity used to dedupe folder spellings. Never sent to the UI.
    pub key: String,
    pub last_organized: u64,
    pub last_instruction: String,
    pub ops_ok: usize,
    pub ops_failed: usize,
    #[serde(default)]
    pub sealed: Vec<SealedFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryStore {
    #[serde(default = "store_version")]
    version: u32,
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

fn store_version() -> u32 {
    STORE_VERSION
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self { version: STORE_VERSION, entries: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedFileView {
    pub path: String,
    pub sealed_at: u64,
    pub restored: bool,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryView {
    pub root: String,
    pub last_organized: u64,
    pub last_instruction: String,
    pub ops_ok: usize,
    pub ops_failed: usize,
    pub sealed: Vec<SealedFileView>,
    pub exists: bool,
}

/// Folder identity: canonicalized so two spellings of the same folder collapse.
fn entry_key(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn load_store(path: &Path) -> HistoryStore {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HistoryStore::default(),
    }
}

/// Atomic write: full temp file first, then replace the store in one step.
fn save_store(path: &Path, store: &HistoryStore) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "history path has no parent folder".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("cannot create history folder: {e}"))?;
    let json = serde_json::to_string_pretty(store).map_err(|e| format!("cannot serialize history: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("cannot write history: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot replace history file: {e}"))
}

/// Move an existing entry to the MRU front or insert a fresh one, then cap the list.
fn upsert(store: &mut HistoryStore, root: &Path, now: u64) -> usize {
    let key = entry_key(root);
    let root_text = root.to_string_lossy().to_string();
    match store.entries.iter().position(|entry| entry.key == key) {
        Some(index) => {
            let mut entry = store.entries.remove(index);
            entry.root = root_text;
            entry.last_organized = now;
            store.entries.insert(0, entry);
        }
        None => {
            store.entries.insert(
                0,
                HistoryEntry {
                    root: root_text,
                    key,
                    last_organized: now,
                    ..HistoryEntry::default()
                },
            );
        }
    }
    store.entries.truncate(MAX_ENTRIES);
    0
}

fn apply_organize(store: &mut HistoryStore, root: &Path, instruction: &str, ok: usize, failed: usize, now: u64) {
    let index = upsert(store, root, now);
    let entry = &mut store.entries[index];
    entry.last_instruction = instruction.to_string();
    entry.ops_ok = ok;
    entry.ops_failed = failed;
}

fn apply_sealed(store: &mut HistoryStore, root: &Path, paths: &[String], now: u64) {
    let index = upsert(store, root, now);
    let entry = &mut store.entries[index];
    for path in paths {
        match entry.sealed.iter_mut().find(|file| file.path.eq_ignore_ascii_case(path)) {
            Some(file) => {
                file.sealed_at = now;
                file.restored = false;
            }
            None => entry.sealed.push(SealedFile {
                path: path.clone(),
                sealed_at: now,
                restored: false,
            }),
        }
    }
}

fn apply_restored(store: &mut HistoryStore, root: &Path, enc_rel: &str) {
    let key = entry_key(root);
    let Some(entry) = store.entries.iter_mut().find(|entry| entry.key == key) else {
        return;
    };
    if let Some(file) = entry.sealed.iter_mut().find(|file| file.path.eq_ignore_ascii_case(enc_rel)) {
        file.restored = true;
    }
}

fn apply_remove(store: &mut HistoryStore, root: &str) -> bool {
    let key = entry_key(Path::new(root));
    let before = store.entries.len();
    store
        .entries
        .retain(|entry| entry.key != key && !entry.root.eq_ignore_ascii_case(root));
    store.entries.len() != before
}

fn views(store: &HistoryStore) -> Vec<HistoryEntryView> {
    store
        .entries
        .iter()
        .map(|entry| {
            let root = Path::new(&entry.root);
            let exists = root.is_dir();
            HistoryEntryView {
                root: entry.root.clone(),
                last_organized: entry.last_organized,
                last_instruction: entry.last_instruction.clone(),
                ops_ok: entry.ops_ok,
                ops_failed: entry.ops_failed,
                sealed: entry
                    .sealed
                    .iter()
                    .map(|file| SealedFileView {
                        path: file.path.clone(),
                        sealed_at: file.sealed_at,
                        restored: file.restored,
                        exists: exists && root.join(&file.path).is_file(),
                    })
                    .collect(),
                exists,
            }
        })
        .collect()
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot resolve the app data folder: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir.join(STORE_FILE))
}

fn read_store(app: &AppHandle) -> Result<HistoryStore, String> {
    let _guard = lock();
    let path = store_path(app)?;
    Ok(load_store(&path))
}

fn mutate_store<F>(app: &AppHandle, action: F) -> Result<(), String>
where
    F: FnOnce(&mut HistoryStore),
{
    let _guard = lock();
    let path = store_path(app)?;
    let mut store = load_store(&path);
    action(&mut store);
    save_store(&path, &store)
}

/// History is best-effort: a store failure must never break the organize flow.
pub fn record_organize(app: &AppHandle, root: &Path, instruction: &str, ok: usize, failed: usize) {
    let now = now_millis();
    if let Err(message) = mutate_store(app, |store| apply_organize(store, root, instruction, ok, failed, now)) {
        eprintln!("epicorganizer: history not saved — {message}");
    }
}

pub fn record_sealed(app: &AppHandle, root: &Path, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let now = now_millis();
    if let Err(message) = mutate_store(app, |store| apply_sealed(store, root, paths, now)) {
        eprintln!("epicorganizer: history not saved — {message}");
    }
}

pub fn mark_restored(app: &AppHandle, root: &Path, enc_rel: &str) {
    if let Err(message) = mutate_store(app, |store| apply_restored(store, root, enc_rel)) {
        eprintln!("epicorganizer: history not saved — {message}");
    }
}

pub fn list(app: &AppHandle) -> Result<Vec<HistoryEntryView>, String> {
    Ok(views(&read_store(app)?))
}

pub fn remove(app: &AppHandle, root: &str) -> Result<(), String> {
    let removed = {
        let _guard = lock();
        let path = store_path(app)?;
        let mut store = load_store(&path);
        let removed = apply_remove(&mut store, root);
        if removed {
            save_store(&path, &store)?;
        }
        removed
    };
    if removed {
        Ok(())
    } else {
        Err("That folder is not in the history.".into())
    }
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    mutate_store(app, |store| *store = HistoryStore::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> HistoryStore {
        HistoryStore::default()
    }

    #[test]
    fn organize_creates_entry_with_details() {
        let mut store = empty();
        apply_organize(&mut store, Path::new("C:/fake/Folder A"), "move images", 3, 1, 1000);
        assert_eq!(store.entries.len(), 1);
        let entry = &store.entries[0];
        assert_eq!(entry.root, "C:/fake/Folder A");
        assert_eq!(entry.last_instruction, "move images");
        assert_eq!(entry.ops_ok, 3);
        assert_eq!(entry.ops_failed, 1);
        assert_eq!(entry.last_organized, 1000);
        assert!(entry.key.contains("folder a"), "key should be case-folded: {}", entry.key);
    }

    #[test]
    fn organize_upserts_same_folder_case_insensitively() {
        let mut store = empty();
        apply_organize(&mut store, Path::new("C:/Fake/FolderA"), "first", 1, 0, 1000);
        apply_organize(&mut store, Path::new("c:/fake/foldera"), "second", 2, 1, 2000);
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].last_instruction, "second");
        assert_eq!(store.entries[0].ops_ok, 2);
        assert_eq!(store.entries[0].last_organized, 2000);
    }

    #[test]
    fn mru_order_and_cap_evicts_oldest() {
        let mut store = empty();
        for index in 0..MAX_ENTRIES + 5 {
            let root = format!("C:/fake/f{index:03}");
            apply_organize(&mut store, Path::new(&root), "x", 0, 0, index as u64);
        }
        assert_eq!(store.entries.len(), MAX_ENTRIES);
        assert_eq!(store.entries[0].root, format!("C:/fake/f{:03}", MAX_ENTRIES + 4));
        assert!(store.entries.iter().all(|entry| entry.root != "C:/fake/f000"));
        apply_organize(&mut store, Path::new("C:/fake/f010"), "again", 1, 0, 9999);
        assert_eq!(store.entries[0].root, "C:/fake/f010");
        assert_eq!(store.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn sealed_dedupes_and_resets_restored_on_reencrypt() {
        let mut store = empty();
        apply_sealed(
            &mut store,
            Path::new("C:/fake/Folder"),
            &["a.txt.enc".to_string(), "a.txt.enc".to_string()],
            10,
        );
        assert_eq!(store.entries[0].sealed.len(), 1);
        apply_restored(&mut store, Path::new("C:/fake/Folder"), "A.TXT.ENC");
        assert!(store.entries[0].sealed[0].restored);
        apply_sealed(&mut store, Path::new("C:/fake/Folder"), &["a.txt.enc".to_string()], 20);
        assert_eq!(store.entries[0].sealed.len(), 1);
        assert!(!store.entries[0].sealed[0].restored);
        assert_eq!(store.entries[0].sealed[0].sealed_at, 20);
    }

    #[test]
    fn restore_and_remove_match_case_insensitively_and_ignore_unknown() {
        let mut store = empty();
        apply_sealed(&mut store, Path::new("C:/fake/Folder"), &["a.txt.enc".to_string()], 10);
        apply_restored(&mut store, Path::new("C:/fake/Folder"), "missing.enc");
        assert!(!store.entries[0].sealed[0].restored);
        apply_restored(&mut store, Path::new("C:/fake/Other"), "a.txt.enc");
        assert_eq!(store.entries.len(), 1);
        apply_organize(&mut store, Path::new("C:/fake/Second"), "x", 0, 0, 1);
        assert!(apply_remove(&mut store, "c:/FAKE/folder"));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].root, "C:/fake/Second");
        assert!(!apply_remove(&mut store, "C:/fake/nothing"));
    }

    #[test]
    fn views_flag_missing_folders_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("present");
        fs::create_dir(&present).unwrap();
        fs::write(present.join("sealed.txt.enc"), b"x").unwrap();

        let mut store = empty();
        apply_organize(&mut store, Path::new("C:/fake/missing-folder"), "go", 1, 0, 1);
        apply_organize(&mut store, &present, "go", 2, 0, 2);
        apply_sealed(
            &mut store,
            &present,
            &["sealed.txt.enc".to_string(), "gone.txt.enc".to_string()],
            2,
        );

        let views = views(&store);
        assert_eq!(views.len(), 2);
        let present_view = views.iter().find(|view| view.root == present.to_string_lossy()).unwrap();
        let missing_view = views.iter().find(|view| view.root == "C:/fake/missing-folder").unwrap();
        assert!(present_view.exists);
        assert!(!missing_view.exists);
        let sealed = present_view.sealed.iter().find(|file| file.path == "sealed.txt.enc").unwrap();
        let gone = present_view.sealed.iter().find(|file| file.path == "gone.txt.enc").unwrap();
        assert!(sealed.exists);
        assert!(!gone.exists);
    }

    #[test]
    fn corrupt_store_loads_empty_and_missing_store_loads_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STORE_FILE);
        let missing = load_store(&path);
        assert!(missing.entries.is_empty());
        fs::write(&path, "{ not json").unwrap();
        let corrupt = load_store(&path);
        assert!(corrupt.entries.is_empty());
        assert_eq!(corrupt.version, STORE_VERSION);
    }

    #[test]
    fn save_and_load_round_trip_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STORE_FILE);
        let mut first = empty();
        apply_organize(&mut first, Path::new("C:/fake/One"), "one", 1, 0, 1);
        save_store(&path, &first).unwrap();
        let mut second = empty();
        apply_organize(&mut second, Path::new("C:/fake/Two"), "two", 2, 0, 2);
        apply_sealed(&mut second, Path::new("C:/fake/Two"), &["a.enc".to_string()], 3);
        save_store(&path, &second).unwrap();

        let loaded = load_store(&path);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].last_instruction, "two");
        assert_eq!(loaded.entries[0].sealed[0].path, "a.enc");
        assert!(!path.with_extension("json.tmp").exists());
    }
}
