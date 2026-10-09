use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::agent::{self, OrganizePlan};
use crate::ollama::OllamaClient;
use crate::tools::{self, ExecutionReport, OperationRequest, OperationResult, ScopedRoot};

#[derive(Clone, Serialize)]
struct ProgressEvent {
    message: String,
}

fn emit_progress(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit("organize-progress", ProgressEvent { message: message.into() });
}

#[tauri::command]
pub async fn plan_organize(app: AppHandle, root: String, instruction: String) -> Result<OrganizePlan, String> {
    let instruction = instruction.trim().to_string();
    if instruction.is_empty() {
        return Err("Type what you want done with this folder first.".into());
    }
    let client = OllamaClient::new()?;
    let progress = move |message: String| emit_progress(&app, message);
    agent::plan(&client, &PathBuf::from(&root), &instruction, &progress).await
}

#[tauri::command]
pub async fn execute_plan(
    app: AppHandle,
    root: String,
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
