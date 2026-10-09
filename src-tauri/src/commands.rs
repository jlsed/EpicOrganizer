use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::agent::{self, OrganizePlan};
use crate::crypto;
use crate::detect::{self, DetectionReport};
use crate::history;
use crate::ollama::OllamaClient;
use crate::tools::{self, ExecutionReport, OperationRequest, OperationResult, ScopedRoot};

#[derive(Clone, Serialize)]
struct ProgressEvent {
    message: String,
}

fn emit_progress(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit("organize-progress", ProgressEvent { message: message.into() });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[tauri::command]
pub fn list_folder_contents(root: String) -> Result<Vec<FolderEntry>, String> {
    let scoped = ScopedRoot::new(&PathBuf::from(&root))?;
    let read_dir = std::fs::read_dir(scoped.canonical())
        .map_err(|e| format!("cannot read folder: {e}"))?;

    let mut entries = Vec::new();
    for entry in read_dir.filter_map(Result::ok) {
        if let Ok(file_type) = entry.file_type() {
            let path = scoped.display_rel(&entry.path());
            let name = entry.file_name().to_string_lossy().to_string();
            let size = if file_type.is_file() {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            entries.push(FolderEntry {
                name,
                path,
                is_dir: file_type.is_dir(),
                size,
            });
        }
    }
    entries.sort_by(|a, b| (!a.is_dir, a.name.to_lowercase()).cmp(&(!b.is_dir, b.name.to_lowercase())));
    Ok(entries)
}

use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Default)]
pub struct PlanCancelState(pub Arc<Mutex<Option<watch::Sender<bool>>>>);

#[tauri::command]
pub async fn plan_organize(
    app: AppHandle,
    state: tauri::State<'_, PlanCancelState>,
    root: String,
    instruction: String,
) -> Result<OrganizePlan, String> {
    let instruction = instruction.trim().to_string();
    if instruction.is_empty() {
        return Err("Type what you want done with this folder first.".into());
    }
    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(cancel_tx);
    }
    let client = OllamaClient::new()?;
    let progress = move |message: String| emit_progress(&app, message);
    let result = agent::plan(&client, &PathBuf::from(&root), &instruction, &progress, cancel_rx).await;
    {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
    result
}

#[tauri::command]
pub fn cancel_plan(state: tauri::State<'_, PlanCancelState>) -> Result<(), String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(ref sender) = *guard {
        let _ = sender.send(true);
    }
    Ok(())
}

#[tauri::command]
pub async fn execute_plan(
    app: AppHandle,
    root: String,
    instruction: String,
    operations: Vec<OperationRequest>,
) -> Result<ExecutionReport, String> {
    let scoped = ScopedRoot::new(&PathBuf::from(&root))?;
    let mut results: Vec<OperationResult> = Vec::with_capacity(operations.len());
    for operation in &operations {
        let description = tools::describe(&operation.tool, &operation.arguments);
        emit_progress(&app, format!("Executing — {description}"));
        let result = match tools::execute_operation(&scoped, &operation.tool, &operation.arguments) {
            Ok(message) => OperationResult { description, ok: true, message },
            Err(message) => OperationResult { description, ok: false, message },
        };
        results.push(result);
    }
    let ok_count = results.iter().filter(|result| result.ok).count();
    let failed_count = results.len() - ok_count;
    history::record_organize(&app, &PathBuf::from(&root), instruction.trim(), ok_count, failed_count);
    emit_progress(&app, format!("Finished — {ok_count} succeeded, {failed_count} failed."));
    Ok(ExecutionReport { results, ok_count, failed_count })
}

#[tauri::command]
pub async fn check_ollama() -> Result<String, String> {
    let client = OllamaClient::new()?;
    let wanted = client.model().to_string();
    let models = client.list_models().await?;
    let found = models.iter().any(|name| name == &wanted || name.starts_with(&format!("{wanted}:")));
    if found {
        client.warm_up().await?;
        Ok(format!("Local AI ready — {wanted} loaded via Ollama."))
    } else {
        let list = if models.is_empty() { "no models installed".to_string() } else { models.join(", ") };
        Err(format!("Model '{wanted}' is not installed in Ollama ({list})."))
    }
}

#[derive(Clone, Serialize)]
struct ConfidentialProgressEvent {
    message: String,
}

fn emit_confidential_progress(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit(
        "confidential-progress",
        ConfidentialProgressEvent { message: message.into() },
    );
}

/// Hybrid scan: deterministic rules always run; the local model's verdict is
/// merged in when Ollama is reachable and degrades to a warning when it is not.
#[tauri::command]
pub async fn detect_confidential(app: AppHandle, root: String) -> Result<DetectionReport, String> {
    let client = OllamaClient::new().ok();
    let progress = move |message: String| emit_confidential_progress(&app, message);
    detect::scan(client.as_ref(), &PathBuf::from(&root), &progress).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptFileResult {
    pub path: String,
    pub output: Option<String>,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptReport {
    pub results: Vec<EncryptFileResult>,
    pub ok_count: usize,
    pub failed_count: usize,
}

fn encrypt_one(scoped: &ScopedRoot, rel: &str, passphrase: &str) -> EncryptFileResult {
    let failure = |message: String| EncryptFileResult {
        path: rel.to_string(),
        output: None,
        ok: false,
        message,
    };
    let path = match scoped.resolve_existing(rel) {
        Ok(path) => path,
        Err(message) => return failure(message),
    };
    if !path.is_file() {
        return failure("not a file".to_string());
    }
    match crypto::encrypt_file(&path, passphrase) {
        Ok(outcome) => {
            let message = if outcome.plaintext_removed {
                format!("Encrypted — {} byte(s) sealed, original removed.", outcome.bytes)
            } else {
                "Encrypted, but the original could not be deleted — remove it manually.".to_string()
            };
            EncryptFileResult {
                path: rel.to_string(),
                output: Some(scoped.display_rel(&outcome.output)),
                ok: true,
                message,
            }
        }
        Err(message) => failure(message),
    }
}

/// Explicit, user-approved encryption of selected files. The passphrase never
/// reaches the model and is zeroized after the command returns.
#[tauri::command]
pub async fn encrypt_files(
    app: AppHandle,
    root: String,
    paths: Vec<String>,
    passphrase: String,
) -> Result<EncryptReport, String> {
    if passphrase.is_empty() {
        return Err("Enter a passphrase first.".into());
    }
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let scoped = ScopedRoot::new(&PathBuf::from(&root))?;

    let mut unique: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        let rel = path.trim().replace('\\', "/");
        if !rel.is_empty() && seen.insert(rel.to_ascii_lowercase()) {
            unique.push(rel);
        }
    }
    if unique.is_empty() {
        return Err("Select at least one file to encrypt.".into());
    }

    let mut results = Vec::with_capacity(unique.len());
    for rel in unique {
        emit_confidential_progress(&app, format!("Encrypting '{rel}'…"));
        results.push(encrypt_one(&scoped, &rel, &passphrase));
    }
    let ok_count = results.iter().filter(|result| result.ok).count();
    let failed_count = results.len() - ok_count;
    let sealed: Vec<String> = results
        .iter()
        .filter(|result| result.ok)
        .filter_map(|result| result.output.clone())
        .collect();
    history::record_sealed(&app, &PathBuf::from(&root), &sealed);
    emit_confidential_progress(
        &app,
        format!("Encryption finished — {ok_count} succeeded, {failed_count} failed."),
    );
    Ok(EncryptReport { results, ok_count, failed_count })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptFileResult {
    pub path: String,
    pub output: String,
    pub ok: bool,
    pub message: String,
}

/// Decrypt one `.enc` file back to its original name (round-trip proof). The
/// encrypted copy is kept; a wrong passphrase fails cleanly with no output.
#[tauri::command]
pub async fn decrypt_file(
    app: AppHandle,
    root: String,
    path: String,
    passphrase: String,
) -> Result<DecryptFileResult, String> {
    if passphrase.is_empty() {
        return Err("Enter the passphrase first.".into());
    }
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let scoped = ScopedRoot::new(&PathBuf::from(&root))?;
    let rel = path.trim().replace('\\', "/");
    if !rel.to_ascii_lowercase().ends_with(".enc") {
        return Err("Only .enc files can be decrypted.".into());
    }
    let file = scoped.resolve_existing(&rel)?;
    let outcome = crypto::decrypt_file(&file, &passphrase)?;
    history::mark_restored(&app, &PathBuf::from(&root), &rel);
    Ok(DecryptFileResult {
        path: rel,
        output: scoped.display_rel(&outcome.output),
        ok: true,
        message: format!(
            "Restored {} byte(s); the encrypted copy was kept.",
            outcome.bytes
        ),
    })
}

#[tauri::command]
pub async fn list_history(app: AppHandle) -> Result<Vec<history::HistoryEntryView>, String> {
    history::list(&app)
}

#[tauri::command]
pub async fn remove_history(app: AppHandle, root: String) -> Result<(), String> {
    history::remove(&app, &root)
}

#[tauri::command]
pub async fn clear_history(app: AppHandle) -> Result<(), String> {
    history::clear(&app)
}
