pub use filemelon_core::{core, repository};
use crate::core::{Core, model::{Rule, RuleRun}};
use std::sync::Arc;
use tauri::{Manager, State, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
fn native_file_dialog(app: &tauri::AppHandle) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    let mut dialog = app.dialog().file();
    if let Some(window) = app.get_webview_window("main") { dialog = dialog.set_parent(&window); }
    dialog
}
struct AppState { core: Arc<Core> }
#[tauri::command]
async fn list_rules(state: State<'_, AppState>) -> Result<Vec<Rule>, String> { state.core.repository.rules.list().await }
#[tauri::command]
async fn save_rule(state: State<'_, AppState>, rule: Rule) -> Result<i64, String> {
    let _guard = state.core.gate.lock().await;
    let id = state.core.repository.rules.save(&rule).await?; state.core.invalidate_schedule(id); Ok(id)
}
#[tauri::command]
async fn set_rule_enabled(state: State<'_, AppState>, id: i64, enabled: bool) -> Result<(), String> {
    let _guard = state.core.gate.lock().await;
    state.core.repository.rules.set_enabled(id, enabled).await?; state.core.invalidate_schedule(id); Ok(())
}
#[tauri::command]
async fn run_now(state: State<'_, AppState>, id: i64) -> Result<Vec<RuleRun>, String> { state.core.run_now(id).await }
#[tauri::command]
async fn run_history(state: State<'_, AppState>) -> Result<Vec<RuleRun>, String> { state.core.repository.history().await }
#[tauri::command]
fn autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> { app.autolaunch().is_enabled().map_err(|e| e.to_string()) }
#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() }.map_err(|e| e.to_string())
}
#[tauri::command]
fn active_run(state: State<'_, AppState>) -> Option<core::ActiveRunStatus> { state.core.active_run() }
#[tauri::command]
fn stop_run(state: State<'_, AppState>, run_id: i64) -> Result<(), String> { state.core.cancel_run(run_id) }
#[tauri::command]
async fn delete_rule(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let _guard = state.core.gate.lock().await;
    state.core.repository.rules.delete_disabled(id).await?; state.core.invalidate_schedule(id); Ok(())
}
#[tauri::command]
async fn next_runs(state: State<'_, AppState>) -> Result<Vec<core::scheduler::NextRun>, String> { state.core.next_runs().await }
#[tauri::command]
async fn pick_directory(app: tauri::AppHandle, current: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut picker = native_file_dialog(&app).set_title("Выберите папку");
        if let Some(current) = current {
            if let Some(parent) = std::path::Path::new(&current).ancestors().find(|p| p.is_dir()) { picker = picker.set_directory(parent); }
        }
        picker.blocking_pick_folder().map(|path| path.into_path().map(|p| p.to_string_lossy().into_owned()).map_err(|e|e.to_string())).transpose()
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn export_rules(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let json = { let _guard = state.core.gate.lock().await; state.core.repository.rules.export_json().await? };
    tauri::async_runtime::spawn_blocking(move || {
        let selected = native_file_dialog(&app).set_title("Экспорт правил Filemelon").add_filter("JSON", &["json"]).set_file_name("filemelon-rules.json").blocking_save_file();
        let Some(selected) = selected else { return Ok(None); };
        let path = selected.into_path().map_err(|e|e.to_string())?;
        std::fs::write(&path, json).map_err(|e|e.to_string())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn import_rules(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Option<repository::rules::ImportResult>, String> {
    let json = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        use std::io::Read;
        let selected = native_file_dialog(&app).set_title("Импорт правил Filemelon").add_filter("JSON", &["json"]).blocking_pick_file();
        let Some(selected) = selected else { return Ok(None); };
        let path = selected.into_path().map_err(|e|e.to_string())?;
        let mut json = String::new();
        std::fs::File::open(path).map_err(|e|e.to_string())?.take(5 * 1024 * 1024 + 1).read_to_string(&mut json).map_err(|e|e.to_string())?;
        Ok(Some(json))
    }).await.map_err(|e|e.to_string())??;
    let Some(json) = json else { return Ok(None); };
    let _guard = state.core.gate.lock().await;
    state.core.repository.rules.import_json(&json).await.map(Some)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .invoke_handler(tauri::generate_handler![list_rules,next_runs,export_rules,import_rules,pick_directory,save_rule,delete_rule,set_rule_enabled,run_now,run_history,active_run,stop_run,autostart_enabled,set_autostart])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let core = tauri::async_runtime::block_on(Core::open(data_dir)).map_err(std::io::Error::other)?;
            app.manage(AppState { core: core.clone() });
            let show = MenuItem::with_id(app, "show", "Открыть Filemelon", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Выйти из Filemelon", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app.default_window_icon().ok_or_else(|| std::io::Error::other("Missing tray icon"))?.clone();
            TrayIconBuilder::new().icon(icon).tooltip("Filemelon").menu(&menu).on_menu_event(|app, event| {
                match event.id.as_ref() {
                    "show" => { if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.set_focus(); } }
                    "quit" => { let handle = app.clone(); tauri::async_runtime::spawn(async move {
                        let state = handle.state::<AppState>();
                        let _guard = state.core.gate.lock().await;
                        handle.exit(0);
                    }); }
                    _ => {}
                }
            }).build(app)?;
            if std::env::args().any(|arg| arg == "--background") { if let Some(window) = app.get_webview_window("main") { window.hide()?; } }
            tauri::async_runtime::spawn(core::scheduler::serve(core));
            Ok(())
        })
        .on_window_event(|window, event| { if let tauri::WindowEvent::CloseRequested { api, .. } = event { api.prevent_close(); let _ = window.hide(); } })
        .run(tauri::generate_context!())
        .expect("error while running Filemelon");
}








