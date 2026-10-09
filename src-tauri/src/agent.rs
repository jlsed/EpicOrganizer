use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::ollama::{ChatMessage, OllamaClient, ToolCall};
use crate::tools::{self, ProposedOperation, ScopedRoot};

const MAX_INSPECT_STEPS: usize = 8;
const MAX_PROPOSED_OPERATIONS: usize = 100;
const INSPECT_NUM_PREDICT: u32 = 384;
const PROPOSE_NUM_PREDICT: u32 = 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePlan {
    pub root: String,
    pub instruction: String,
    pub model: String,
    pub summary: String,
    pub operations: Vec<ProposedOperation>,
    pub warnings: Vec<String>,
}

/// Two-phase loop: inspect the folder with read-only tools, then ask a single
/// extra turn for the mutation tool calls that implement the plan. Mutations
/// are only validated here — nothing touches disk until the user approves.
pub async fn plan<F: Fn(String)>(
    client: &OllamaClient,
    root: &Path,
    instruction: &str,
    progress: &F,
) -> Result<OrganizePlan, String> {
    let scoped = ScopedRoot::new(root)?;
    let root_display = root.display().to_string();
    progress("Checking the chosen folder…".to_string());

    let mut messages = vec![
        ChatMessage::system(understand_prompt(&root_display)),
        ChatMessage::user(instruction),
    ];
    let read_tools = tools::read_tool_schemas();
    let mut warnings = Vec::new();
    let mut summary = String::new();
    let mut steps = 0usize;
    let mut read_paths: BTreeSet<String> = BTreeSet::new();

    loop {
        if steps >= MAX_INSPECT_STEPS {
            warnings.push(format!("Stopped inspecting after {MAX_INSPECT_STEPS} steps."));
            break;
        }
        let response = client.chat(&messages, &read_tools, INSPECT_NUM_PREDICT).await?;
        let calls = response.tool_calls.clone().unwrap_or_default();
        if let Some(content) = response.content.as_deref() {
            let cleaned = sanitize_summary(content);
            if !cleaned.is_empty() {
                summary = cleaned;
            }
        }
        messages.push(response);
        if calls.is_empty() {
            break;
        }
        steps += 1;
        for call in &calls {
            progress(format!("Inspecting — {}", call.function.name));
            let content = run_inspect_tool(&scoped, call, &mut read_paths)
                .unwrap_or_else(|error| format!("error: {error}"));
            messages.push(ChatMessage::tool(&call.function.name, content));
        }
    }

    if summary.is_empty() {
        summary = "The model did not write a summary for this plan.".to_string();
        warnings.push("No plan summary was produced.".to_string());
    }

    progress("Proposing file operations…".to_string());
    messages.push(ChatMessage::user(PROPOSE_INSTRUCTION));
    let proposal = client.chat(&messages, &tools::mutation_tool_schemas(), PROPOSE_NUM_PREDICT).await?;
    let calls = proposal.tool_calls.unwrap_or_default();
    if calls.len() > MAX_PROPOSED_OPERATIONS {
        warnings.push(format!("Kept the first {MAX_PROPOSED_OPERATIONS} proposed operations."));
    }
    let operations: Vec<ProposedOperation> = calls
        .into_iter()
        .take(MAX_PROPOSED_OPERATIONS)
        .map(|call| to_proposed_operation(&scoped, call))
        .collect();
    if operations.is_empty() {
        warnings.push("The model proposed no file operations.".to_string());
    }

    Ok(OrganizePlan {
        root: root_display,
        instruction: instruction.to_string(),
        model: client.model().to_string(),
        summary,
        operations,
        warnings,
    })
}

fn run_inspect_tool(
    root: &ScopedRoot,
    call: &ToolCall,
    read_paths: &mut BTreeSet<String>,
) -> Result<String, String> {
    let args = call.function.arguments_object()?;
    match call.function.name.as_str() {
        tools::TOOL_LIST_FILES => tools::execute_list_files(root, &args),
        tools::TOOL_READ_FILE => {
            let rel = normalize_rel(
                args.get("path").and_then(|value| value.as_str()).unwrap_or_default(),
            );
            if !rel.is_empty() && read_paths.contains(&rel) {
                return Ok(format!(
                    "[already read '{rel}' earlier in this session — reuse that content and stop inspecting]"
                ));
            }
            let content = tools::execute_read_file(root, &args)?;
            if !rel.is_empty() {
                read_paths.insert(rel);
            }
            Ok(content)
        }
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn normalize_rel(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/").to_lowercase();
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

/// Small models sometimes echo raw JSON tool-call drafts in their text.
/// Drop those lines so the plan summary stays readable.
fn sanitize_summary(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !(line.starts_with('{') && line.ends_with('}')))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn to_proposed_operation(root: &ScopedRoot, call: ToolCall) -> ProposedOperation {
    let raw_arguments = call.function.arguments.clone();
    let description = tools::describe(&call.function.name, &raw_arguments);
    match call.function.arguments_object() {
        Ok(args) => match tools::validate_operation(root, &call.function.name, &args) {
            Ok(_) => ProposedOperation {
                tool: call.function.name,
                arguments: args,
                description,
                valid: true,
                reason: None,
            },
            Err(reason) => ProposedOperation {
                tool: call.function.name,
                arguments: args,
                description,
                valid: false,
                reason: Some(reason),
            },
        },
        Err(reason) => ProposedOperation {
            tool: call.function.name,
            arguments: raw_arguments,
            description,
            valid: false,
            reason: Some(reason),
        },
    }
}

fn understand_prompt(root: &str) -> String {
    format!(
        "You are EpicOrganizer, a meticulous local file-organization assistant.\n\
         The user selected this ROOT folder:\n{root}\n\n\
         Rules:\n\
         - All paths you pass to tools MUST be relative to the ROOT, for example \"notes.txt\" or \"Images/photo.jpg\".\n\
         - NEVER use absolute paths, drive letters, or \"..\".\n\
         - Call list_files before mentioning any file or folder; never guess paths.\n\
         - Use read_file only when a file's content helps decide where it belongs. Never read the same file twice.\n\
         - Images, archives and other binary files return no text — skip them, do not retry.\n\
         - Inspect the folder, then reply with a short plan summary: which files/folders to organize and where.\n\
         - Only organize what the instruction asks for. Keep file names and extensions unchanged unless asked.\n\
         - Nothing has been changed yet — never claim a file was moved or created.\n\
         - Keep the summary under 80 words."
    )
}

const PROPOSE_INSTRUCTION: &str = "Now emit ONLY the operation tool calls that implement your plan. \
Use create_folder and move_file. Every destination must include the full path relative to the root, \
including the original file name and extension. Keep file names and extensions exactly as they are — \
do not change, translate, or strip them. Only use rename_file if the user explicitly asked for renaming. \
Each file must appear in at most one operation. Do not explain anything in text — just call the tools.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_root_and_relative_path_rule() {
        let prompt = understand_prompt("E:\\Demo");
        assert!(prompt.contains("E:\\Demo"));
        assert!(prompt.contains("relative to the ROOT"));
    }

    #[test]
    fn sanitize_summary_drops_echoed_json_lines() {
        let noisy = "Here is the plan:\n\n{\"name\": \"list_files\", \"parameters\": {\"path\": \"\"}}\n1. Move images to Images/.";
        assert_eq!(sanitize_summary(noisy), "Here is the plan: 1. Move images to Images/.");
    }

    /// Live smoke test: needs `ollama serve` with llama3.2:3b on localhost.
    /// Run with: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires a running Ollama with the configured model"]
    fn plans_and_executes_against_live_ollama() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("IMG_001.jpg"), [0u8, 1, 2, 3]).unwrap();
        fs::write(dir.path().join("screenshot.png"), [0u8, 1, 2, 3]).unwrap();
        fs::write(dir.path().join("notes.txt"), b"shopping list: milk, eggs").unwrap();
        fs::write(dir.path().join("report.md"), b"# Q3 report").unwrap();

        let client = OllamaClient::new().unwrap();
        let plan = tauri::async_runtime::block_on(plan(
            &client,
            dir.path(),
            "Move all images into a new folder called Images.",
            &|message: String| eprintln!("[progress] {message}"),
        ))
        .unwrap();

        eprintln!("[summary] {}", plan.summary);
        for operation in &plan.operations {
            eprintln!(
                "[op] valid={} {} {}",
                operation.valid,
                operation.description,
                operation.reason.as_deref().unwrap_or("")
            );
        }
        assert!(!plan.summary.is_empty());

        let scoped = ScopedRoot::new(dir.path()).unwrap();
        for operation in plan.operations.iter().filter(|op| op.valid) {
            let result = tools::execute_operation(&scoped, &operation.tool, &operation.arguments);
            assert!(result.is_ok(), "execution failed for '{}': {result:?}", operation.description);
        }
        let moved = fs::read_dir(dir.path().join("Images"))
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0);
        eprintln!("[result] {moved} file(s) inside Images/");
    }
}
