mod audio;
mod commands;
mod errors;
mod storage;
mod stt;
mod system;

use commands::AppState;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            *commands::GLOBAL_APP_HANDLE.lock().unwrap() = Some(app_handle);

            // Spawn global mouse and keyboard hook listener
            commands::start_hook_thread();

            let state = AppState::new(app.handle())?;
            let always_on = state.settings.lock().unwrap().always_on;
            app.manage(state);

            // 1. Create native system tray menu items
            let settings_i = MenuItem::with_id(app, "settings", "Настройки (Settings)", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Выйти (Exit)", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;

            // 2. Build the System Tray defensively
            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::new()
                    .icon(icon.clone())
                    .menu(&menu)
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "settings" => {
                                let app_clone = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = crate::commands::open_settings_window(app_clone).await;
                                });
                            }
                            "quit" => {
                                crate::commands::stop_hook_thread();
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                            if let Some(main_window) = tray.app_handle().get_webview_window("main") {
                                if let Ok(is_visible) = main_window.is_visible() {
                                    if is_visible {
                                        let _ = main_window.hide();
                                    } else {
                                        let _ = main_window.show();
                                        let _ = main_window.unminimize();
                                        let _ = main_window.set_focus();
                                    }
                                }
                            }
                        }
                    })
                    .build(app)?;
            }

            // Hide the widget at startup if hold-hotkey mode is active
            if !always_on {
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.hide();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    commands::stop_hook_thread();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::get_recording_state,
            commands::transcribe_audio,
            commands::get_model_status,
            commands::set_model_path,
            commands::open_models_folder,
            commands::start_window_drag,
            commands::close_current_window,
            commands::open_settings_window,
            commands::list_whisper_models,
            commands::download_whisper_model,
            commands::get_download_progress,
            commands::cancel_download,
            commands::delete_whisper_model,
            commands::save_transcript,
            commands::get_history,
            commands::delete_history_item,
            commands::export_txt,
            commands::write_clipboard,
            commands::get_hotkey_settings,
            commands::update_hotkey_settings,
            commands::inject_text,
            commands::start_recording_hotkey,
            commands::cancel_recording_hotkey,
            commands::log_message,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Ferrofluid Voice");
}
