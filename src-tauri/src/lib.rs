mod agent;
mod commands;
mod ollama;
mod tools;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::plan_organize,
            commands::execute_plan,
            commands::check_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
