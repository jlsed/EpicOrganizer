mod agent;
mod commands;
mod crypto;
mod detect;
mod extract;
mod history;
mod ollama;
mod tools;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::PlanCancelState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_folder_contents,
            commands::plan_organize,
            commands::cancel_plan,
            commands::execute_plan,
            commands::check_ollama,
            commands::detect_confidential,
            commands::encrypt_files,
            commands::decrypt_file,
            commands::list_history,
            commands::remove_history,
            commands::clear_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
