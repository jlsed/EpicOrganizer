use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TOOL_LIST_FILES: &str = "list_files";
pub const TOOL_READ_FILE: &str = "read_file";
pub const TOOL_CREATE_FOLDER: &str = "create_folder";
pub const TOOL_MOVE_FILE: &str = "move_file";
pub const TOOL_RENAME_FILE: &str = "rename_file";

const MAX_LIST_DEPTH: u32 = 2;
const MAX_LIST_ENTRIES: usize = 200;
const CONTENT_CHAR_LIMIT: usize = 4096;

/// A file operation proposed by the model, validated and ready for the user to review.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedOperation {
    pub tool: String,
    pub arguments: Value,
    pub description: String,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The minimal payload the frontend sends back for execution (never trusted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRequest {
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub description: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    pub results: Vec<OperationResult>,
    pub ok_count: usize,
    pub failed_count: usize,
}

pub fn read_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": TOOL_LIST_FILES,
                "description": "List files and folders under the chosen root folder.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Folder to list, relative to the root. Omit to list the root folder." },
                        "depth": { "type": "integer", "description": "How many levels to list, 1 or 2 (default 1)." }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_READ_FILE,
                "description": "Read the beginning of a text, PDF, or Word (.docx) document. The path must be relative to the root.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File to read, relative to the root." }
                    },
                    "required": ["path"]
                }
            }
        }),
    ]
}

pub fn mutation_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": TOOL_CREATE_FOLDER,
                "description": "Create a folder. The path must be relative to the root.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Folder path to create, relative to the root." }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_MOVE_FILE,
                "description": "Move a file or folder to a new location. Both paths must be relative to the root.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Existing path to move, relative to the root." },
                        "destination": { "type": "string", "description": "Full destination path including the file name, relative to the root." }
                    },
                    "required": ["source", "destination"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_RENAME_FILE,
                "description": "Rename a file or folder in place. The new name is a plain name, not a path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Existing path to rename, relative to the root." },
                        "new_name": { "type": "string", "description": "New file or folder name only (no slashes)." }
                    },
                    "required": ["path", "new_name"]
                }
            }
        }),
    ]
}

pub fn describe(tool: &str, args: &Value) -> String {
    let get = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("?");
    match tool {
        TOOL_CREATE_FOLDER => format!("Create folder '{}'", get("path")),
        TOOL_MOVE_FILE => format!("Move '{}' → '{}'", get("source"), get("destination")),
        TOOL_RENAME_FILE => format!("Rename '{}' → '{}'", get("path"), get("new_name")),
        TOOL_LIST_FILES => format!("List files in '{}'", get("path")),
        TOOL_READ_FILE => format!("Read '{}'", get("path")),
        other => other.to_string(),
    }
}

fn clean_canonical(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{stripped}"))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

pub fn validate_safe_root(canonical: &Path) -> Result<(), String> {
    let clean = clean_canonical(canonical);

    // 1. Block Drive Roots (e.g. C:\, D:\, /)
    let normal_components = clean
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    if normal_components == 0 {
        return Err(format!(
            "Drive roots ('{}') cannot be organized directly. Please select a specific folder inside the drive.",
            clean.display()
        ));
    }

    // 2. Allow subdirectories inside standard Temp (used by unit tests and temporary scratch folders)
    if let Ok(temp) = std::env::temp_dir().canonicalize() {
        let clean_temp = clean_canonical(&temp);
        if clean.starts_with(&clean_temp) && clean != clean_temp {
            return Ok(());
        }
    }

    // 3. Block User Profile Root directly (e.g. C:\Users\username or /home/username)
    if let Ok(profile) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if let Ok(profile_can) = PathBuf::from(profile).canonicalize() {
            let clean_profile = clean_canonical(&profile_can);
            if clean.to_string_lossy().eq_ignore_ascii_case(&clean_profile.to_string_lossy()) {
                return Err(
                    "Your entire user profile directory cannot be organized directly. Please select a specific subfolder (e.g. Downloads, Documents, Desktop, or a project folder).".into(),
                );
            }
        }
    }

    // 4. Block System, Program, and AppData directories (and their subfolders)
    let mut blocked_prefixes: Vec<PathBuf> = Vec::new();

    // Windows system variables
    for var in &[
        "SystemRoot",
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        if let Ok(val) = std::env::var(var) {
            if let Ok(can) = PathBuf::from(val).canonicalize() {
                let clean_p = clean_canonical(&can);
                blocked_prefixes.push(clean_p);
            }
        }
    }

    // AppData root folder (parent of Roaming/Local)
    if let Ok(appdata) = std::env::var("APPDATA") {
        if let Some(parent) = PathBuf::from(appdata).parent() {
            if let Ok(can) = parent.canonicalize() {
                blocked_prefixes.push(clean_canonical(&can));
            }
        }
    }

    // Unix system folders
    for unix_sys in &[
        "/etc", "/usr", "/var", "/sys", "/proc", "/dev", "/boot", "/bin", "/sbin", "/root",
    ] {
        blocked_prefixes.push(PathBuf::from(unix_sys));
    }

    for blocked in blocked_prefixes {
        let clean_str = clean.to_string_lossy().to_lowercase();
        let blocked_str = blocked.to_string_lossy().to_lowercase();
        if clean_str == blocked_str
            || clean_str.starts_with(&format!("{blocked_str}\\"))
            || clean_str.starts_with(&format!("{blocked_str}/"))
        {
            return Err(format!(
                "'{}' is a protected system/application directory and cannot be selected.",
                clean.display()
            ));
        }
    }

    // 4. Block sensitive credential folders (e.g. .ssh, .aws, .gnupg)
    for comp in clean.components() {
        if let Component::Normal(os_str) = comp {
            let lower = os_str.to_string_lossy().to_lowercase();
            if lower == ".ssh" || lower == ".aws" || lower == ".gnupg" {
                return Err(format!(
                    "Sensitive credential directory '{}' is protected and cannot be organized.",
                    clean.display()
                ));
            }
        }
    }

    Ok(())
}

/// Every resolved path must stay inside this root. The root is canonicalized
/// once; existing paths are canonicalized before the prefix check so that `..`
/// or symlinks cannot escape the chosen folder.
#[derive(Debug, Clone)]
pub struct ScopedRoot {
    canonical: PathBuf,
}

impl ScopedRoot {
    pub fn new(root: &Path) -> Result<Self, String> {
        let canonical = root
            .canonicalize()
            .map_err(|e| format!("cannot open folder {}: {e}", root.display()))?;
        if !canonical.is_dir() {
            return Err(format!("{} is not a folder", root.display()));
        }
        validate_safe_root(&canonical)?;
        Ok(Self { canonical })
    }

    pub fn canonical(&self) -> &Path {
        &self.canonical
    }

    pub fn display_rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.canonical)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string())
    }

    /// Resolve a path that must already exist.
    pub fn resolve_existing(&self, rel: &str) -> Result<PathBuf, String> {
        let candidate = self.join_relative(rel)?;
        let canonical = candidate
            .canonicalize()
            .map_err(|_| format!("'{rel}' does not exist"))?;
        self.ensure_within(&canonical)?;
        Ok(canonical)
    }

    /// Resolve a path that must not exist yet (create/move/rename target).
    /// The deepest existing ancestor is canonicalized and scope-checked; the
    /// missing trailing components are appended as plain names.
    pub fn resolve_new(&self, rel: &str) -> Result<PathBuf, String> {
        let candidate = self.join_relative(rel)?;
        if candidate.exists() {
            return Err(format!("'{rel}' already exists"));
        }
        let mut existing = candidate;
        let mut missing = Vec::new();
        loop {
            match existing.canonicalize() {
                Ok(canonical) => {
                    self.ensure_within(&canonical)?;
                    let mut path = canonical;
                    for part in missing.iter().rev() {
                        path = path.join(part);
                    }
                    return Ok(path);
                }
                Err(_) => {
                    let name = existing
                        .file_name()
                        .ok_or_else(|| format!("invalid path '{rel}'"))?
                        .to_os_string();
                    missing.push(name);
                    existing = existing
                        .parent()
                        .ok_or_else(|| format!("invalid path '{rel}'"))?
                        .to_path_buf();
                }
            }
        }
    }

    fn join_relative(&self, rel: &str) -> Result<PathBuf, String> {
        let trimmed = rel.trim();
        if trimmed.is_empty() {
            return Err("path must not be empty".into());
        }
        let path = Path::new(trimmed);
        if path.is_absolute() {
            return Err(format!("'{rel}' must be a path relative to the chosen folder"));
        }
        let mut joined = self.canonical.clone();
        for component in path.components() {
            match component {
                Component::Normal(part) => joined.push(part),
                Component::CurDir => {}
                _ => return Err(format!("'{rel}' is outside the chosen folder")),
            }
        }
        Ok(joined)
    }

    fn ensure_within(&self, canonical: &Path) -> Result<(), String> {
        if canonical.starts_with(&self.canonical) {
            Ok(())
        } else {
            Err("path is outside the chosen folder".into())
        }
    }
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing required argument '{key}'"))
}

fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." {
        return Err("new_name must be a plain file name".into());
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err("new_name contains characters that are not allowed".into());
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("new_name must not end with a dot or a space".into());
    }
    Ok(())
}

pub fn execute_list_files(root: &ScopedRoot, args: &Value) -> Result<String, String> {
    let rel = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(".");
    let depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, u64::from(MAX_LIST_DEPTH)) as u32;

    let start = if rel == "." { root.canonical().to_path_buf() } else { root.resolve_existing(rel)? };
    if !start.is_dir() {
        return Err(format!("'{rel}' is not a folder"));
    }

    let mut entries = Vec::new();
    let mut truncated = false;
    collect_entries(root, &start, depth, &mut entries, &mut truncated)?;

    let shown = if rel == "." { "/" } else { rel };
    let mut out = format!("Contents of '{shown}' (paths relative to the root):");
    if entries.is_empty() {
        out.push_str("\n(empty folder)");
    } else {
        for entry in entries {
            out.push('\n');
            out.push_str(&entry);
        }
    }
    if truncated {
        out.push_str(&format!("\n…[listing truncated at {MAX_LIST_ENTRIES} entries]"));
    }
    Ok(out)
}

fn collect_entries(
    root: &ScopedRoot,
    dir: &Path,
    depth_left: u32,
    out: &mut Vec<String>,
    truncated: &mut bool,
) -> Result<(), String> {
    let read_dir = fs::read_dir(dir).map_err(|e| format!("cannot read '{}': {e}", root.display_rel(dir)))?;
    let mut children: Vec<(PathBuf, fs::FileType)> = read_dir
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_type().ok().map(|file_type| (entry.path(), file_type)))
        .collect();
    children.sort_by(|a, b| {
        let key = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };
        (!a.1.is_dir(), key(&a.0)).cmp(&(!b.1.is_dir(), key(&b.0)))
    });

    for (child, file_type) in children {
        if out.len() >= MAX_LIST_ENTRIES {
            *truncated = true;
            return Ok(());
        }
        let name = root.display_rel(&child);
        if file_type.is_symlink() {
            out.push(format!("[link] {name}"));
        } else if file_type.is_dir() {
            out.push(format!("[dir]  {name}/"));
            if depth_left > 1 {
                collect_entries(root, &child, depth_left - 1, out, truncated)?;
            }
        } else {
            let size = fs::metadata(&child).map(|m| m.len()).unwrap_or(0);
            out.push(format!("[file] {name} ({size} bytes)"));
        }
    }
    Ok(())
}

pub fn execute_read_file(root: &ScopedRoot, args: &Value) -> Result<String, String> {
    let rel = arg_str(args, "path")?;
    let path = root.resolve_existing(rel)?;
    if !path.is_file() {
        return Err(format!("'{rel}' is not a file"));
    }
    let text = match crate::extract::extract_text(&path) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return Ok(format!("[content of '{rel}' is not extracted — unsupported file type]"));
        }
        Err(reason) => {
            return Ok(format!("[content of '{rel}' could not be extracted: {reason}]"));
        }
    };
    let mut chars = text.chars();
    let truncated_text: String = chars.by_ref().take(CONTENT_CHAR_LIMIT).collect();
    let truncated = chars.next().is_some();

    let mut out = format!("--- {rel} ---\n{truncated_text}");
    if truncated {
        out.push_str("\n…[content truncated]");
    }
    Ok(out)
}

/// Validate a mutation the model proposed. Returns the review description.
pub fn validate_operation(root: &ScopedRoot, tool: &str, args: &Value) -> Result<String, String> {
    match tool {
        TOOL_CREATE_FOLDER => {
            let rel = arg_str(args, "path")?;
            root.resolve_new(rel)?;
            Ok(format!("Create folder '{rel}'"))
        }
        TOOL_MOVE_FILE => {
            let source = arg_str(args, "source")?;
            let destination = arg_str(args, "destination")?;
            let src = root.resolve_existing(source)?;
            let dst = root.resolve_new(destination)?;
            if src == dst {
                return Err("source and destination are the same path".into());
            }
            Ok(format!("Move '{source}' → '{destination}'"))
        }
        TOOL_RENAME_FILE => {
            let rel = arg_str(args, "path")?;
            let new_name = arg_str(args, "new_name")?;
            validate_file_name(new_name)?;
            let src = root.resolve_existing(rel)?;
            let parent = src.parent().ok_or_else(|| format!("cannot determine the folder of '{rel}'"))?;
            let dst = parent.join(new_name);
            if dst == src {
                return Err("the new name is the same as the current name".into());
            }
            if dst.exists() {
                return Err(format!("'{new_name}' already exists"));
            }
            Ok(format!("Rename '{rel}' → '{new_name}'"))
        }
        other => Err(format!("unsupported tool '{other}'")),
    }
}

/// Execute a validated operation. Paths are resolved again here so a stale or
/// tampered request can never escape the scope.
pub fn execute_operation(root: &ScopedRoot, tool: &str, args: &Value) -> Result<String, String> {
    match tool {
        TOOL_CREATE_FOLDER => {
            let rel = arg_str(args, "path")?;
            let path = root.resolve_new(rel)?;
            fs::create_dir_all(&path).map_err(|e| format!("could not create folder '{rel}': {e}"))?;
            Ok(format!("Created folder '{rel}'"))
        }
        TOOL_MOVE_FILE => {
            let source = arg_str(args, "source")?;
            let destination = arg_str(args, "destination")?;
            let src = root.resolve_existing(source)?;
            let dst = root.resolve_new(destination)?;
            if src == dst {
                return Err("source and destination are the same path".into());
            }
            fs::rename(&src, &dst).map_err(|e| format!("could not move '{source}' → '{destination}': {e}"))?;
            Ok(format!("Moved '{source}' → '{destination}'"))
        }
        TOOL_RENAME_FILE => {
            let rel = arg_str(args, "path")?;
            let new_name = arg_str(args, "new_name")?;
            validate_file_name(new_name)?;
            let src = root.resolve_existing(rel)?;
            let parent = src.parent().ok_or_else(|| format!("cannot determine the folder of '{rel}'"))?;
            let dst = parent.join(new_name);
            if dst == src {
                return Err("the new name is the same as the current name".into());
            }
            if dst.exists() {
                return Err(format!("'{new_name}' already exists"));
            }
            fs::rename(&src, &dst).map_err(|e| format!("could not rename '{rel}': {e}"))?;
            Ok(format!("Renamed '{rel}' → '{new_name}'"))
        }
        other => Err(format!("unsupported tool '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scoped_root(dir: &tempfile::TempDir) -> ScopedRoot {
        ScopedRoot::new(dir.path()).expect("scoped root")
    }

    fn touch(root: &ScopedRoot, rel: &str) -> PathBuf {
        let path = root.canonical().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        assert!(root.resolve_existing("../outside.txt").is_err());
        assert!(root.resolve_new("../outside.txt").is_err());
        assert!(root.resolve_existing("/etc/passwd").is_err());
        assert!(root.resolve_new("/etc/passwd").is_err());
        assert!(validate_operation(&root, TOOL_CREATE_FOLDER, &json!({ "path": "../escape" })).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_prefixed_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        assert!(root.resolve_new("C:/Windows/Temp/evil").is_err());
    }

    #[test]
    fn resolves_paths_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        touch(&root, "notes/todo.txt");
        let resolved = root.resolve_existing("notes/todo.txt").unwrap();
        assert!(resolved.starts_with(root.canonical()));
        assert_eq!(root.display_rel(&resolved), "notes/todo.txt");
        let fresh = root.resolve_new("new/deep/folder").unwrap();
        assert!(fresh.starts_with(root.canonical()));
        assert!(!fresh.exists());
    }

    #[test]
    fn create_folder_rejects_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        fs::create_dir(root.canonical().join("Images")).unwrap();
        let err = validate_operation(&root, TOOL_CREATE_FOLDER, &json!({ "path": "Images" })).unwrap_err();
        assert!(err.contains("already exists"), "unexpected error: {err}");
    }

    #[test]
    fn move_validation_checks_source_and_destination() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        touch(&root, "a.jpg");
        touch(&root, "b.jpg");
        assert!(validate_operation(
            &root,
            TOOL_MOVE_FILE,
            &json!({ "source": "missing.jpg", "destination": "Images/missing.jpg" })
        )
        .is_err());
        assert!(validate_operation(&root, TOOL_MOVE_FILE, &json!({ "source": "a.jpg", "destination": "b.jpg" })).is_err());
        let description =
            validate_operation(&root, TOOL_MOVE_FILE, &json!({ "source": "a.jpg", "destination": "Images/a.jpg" }))
                .unwrap();
        assert_eq!(description, "Move 'a.jpg' → 'Images/a.jpg'");
    }

    #[test]
    fn rename_validation_rejects_separators_and_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        touch(&root, "a.txt");
        touch(&root, "b.txt");
        assert!(validate_operation(&root, TOOL_RENAME_FILE, &json!({ "path": "a.txt", "new_name": "../a.txt" })).is_err());
        assert!(validate_operation(&root, TOOL_RENAME_FILE, &json!({ "path": "a.txt", "new_name": "sub/a.txt" })).is_err());
        assert!(validate_operation(&root, TOOL_RENAME_FILE, &json!({ "path": "a.txt", "new_name": "b.txt" })).is_err());
        assert!(validate_operation(&root, TOOL_RENAME_FILE, &json!({ "path": "a.txt", "new_name": "c.txt" })).is_ok());
    }

    #[test]
    fn read_file_truncates_long_text_and_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        let long_text = "a".repeat(CONTENT_CHAR_LIMIT + 500);
        fs::write(root.canonical().join("long.txt"), long_text).unwrap();
        let text = execute_read_file(&root, &json!({ "path": "long.txt" })).unwrap();
        assert!(text.contains("[content truncated]"));
        assert!(text.chars().count() < CONTENT_CHAR_LIMIT + 200);

        fs::write(root.canonical().join("image.png"), [0u8, 159, 146, 150]).unwrap();
        let skipped = execute_read_file(&root, &json!({ "path": "image.png" })).unwrap();
        assert!(skipped.contains("not extracted"));
    }

    #[test]
    fn list_files_lists_relative_paths_and_caps_depth() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        touch(&root, "a.txt");
        touch(&root, "Images/photo.jpg");
        let listing = execute_list_files(&root, &json!({})).unwrap();
        assert!(listing.contains("a.txt"));
        assert!(listing.contains("Images/"));
        assert!(!listing.contains("photo.jpg"), "depth 1 must not list nested files");
        let deep = execute_list_files(&root, &json!({ "depth": 2 })).unwrap();
        assert!(deep.contains("Images/photo.jpg"));
    }

    #[test]
    fn execute_move_and_rename_work_and_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let root = scoped_root(&dir);
        touch(&root, "a.txt");
        touch(&root, "keep.txt");
        execute_operation(&root, TOOL_CREATE_FOLDER, &json!({ "path": "docs" })).unwrap();
        execute_operation(&root, TOOL_MOVE_FILE, &json!({ "source": "a.txt", "destination": "docs/a.txt" })).unwrap();
        assert!(root.canonical().join("docs/a.txt").exists());
        assert!(execute_operation(&root, TOOL_MOVE_FILE, &json!({ "source": "keep.txt", "destination": "docs/a.txt" })).is_err());
        execute_operation(&root, TOOL_RENAME_FILE, &json!({ "path": "docs/a.txt", "new_name": "b.txt" })).unwrap();
        assert!(root.canonical().join("docs/b.txt").exists());
    }

    #[test]
    fn safe_root_validation_rejects_protected_locations() {
        // Safe temp directory should succeed
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_safe_root(dir.path()).is_ok());

        // Windows / System dirs if present
        if let Ok(windir) = std::env::var("SystemRoot").or_else(|_| std::env::var("windir")) {
            let path = PathBuf::from(windir);
            if path.exists() {
                assert!(validate_safe_root(&path).is_err(), "SystemRoot must be rejected");
            }
        }

        // AppData if present
        if let Ok(appdata) = std::env::var("APPDATA") {
            let path = PathBuf::from(appdata);
            if path.exists() {
                assert!(validate_safe_root(&path).is_err(), "APPDATA must be rejected");
            }
        }

        // Drive root simulation
        #[cfg(windows)]
        {
            assert!(validate_safe_root(Path::new("C:\\")).is_err(), "Drive root C:\\ must be rejected");
        }
    }
}
