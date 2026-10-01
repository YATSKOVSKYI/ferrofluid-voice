mod audio;
mod commands;
mod errors;
mod storage;
mod stt;
mod system;
mod meetings;
mod instance;

use commands::AppState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn activate_existing_window(app: &AppHandle) {
    for label in ["library", "settings"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = instance::restore_window(&window);
            return;
        }
    }
    show_main_window(app);
}

fn show_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = instance::restore_window(&main_window);
        return;
    }

    if let Some(settings_window) = app.get_webview_window("settings") {
        let _ = instance::restore_window(&settings_window);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _instance_guard = match instance::enter() {
        Ok(Some(guard)) => guard,
        Ok(None) => return,
        Err(error) => {
            instance::report_failure(&error);
            std::process::exit(2);
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|arg| arg == "--library") {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { let _ = commands::open_library_window(app).await; });
                return;
            }
            // Return from WM_COPYDATA immediately; all window work is queued.
            let app_handle = app.clone();
            let _ = app.run_on_main_thread(move || activate_existing_window(&app_handle));
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(meetings::MeetingsState::default())
        .setup(|app| {
            let app_handle = app.handle().clone();
            *commands::GLOBAL_APP_HANDLE.lock().unwrap() = Some(app_handle);

            let state = AppState::new(app.handle())?;
            let always_on = state.settings.lock().unwrap().always_on;
            app.manage(state);
            // Hooks need the loaded binding and managed application state.
            commands::start_hook_thread();
            if std::env::args().any(|arg| arg == "--library") {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(async move { let _ = commands::open_library_window(app).await; });
            }

            // 1. Create native system tray menu items
            let settings_i =
                MenuItem::with_id(app, "settings", "Настройки (Settings)", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Выйти (Exit)", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;

            // 2. Build the System Tray defensively
            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::new()
                    .icon(icon.clone())
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
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
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            ..
                        } = event
                        {
                            if let Some(main_window) = tray.app_handle().get_webview_window("main")
                            {
                                if let Ok(is_visible) = main_window.is_visible() {
                                    if is_visible {
                                        let _ = main_window.hide();
                                    } else {
                                        show_main_window(tray.app_handle());
                                    }
                                }
                            }
                        }
                    })
                    .build(app)?;
            }

            // An explicit launch must always reveal a window. Hold-hotkey mode
            // may hide the widget only when the Library is being opened instead.
            if !always_on && std::env::args().any(|arg| arg == "--library") {
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.hide();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                match event {
                    tauri::WindowEvent::CloseRequested { .. } => {
                        commands::stop_hook_thread();
                        window.app_handle().exit(0);
                    }
                    tauri::WindowEvent::Destroyed => commands::stop_hook_thread(),
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            meetings::meeting_tools_status,
            meetings::setup_meeting_tools,
            meetings::cancel_meeting_job,
            meetings::import_meeting,
            meetings::analyze_meeting,
            meetings::transcribe_meeting,
            meetings::calibrate_meeting,
            meetings::refine_meeting,
            meetings::list_meetings,
            meetings::save_meeting,
            meetings::export_meeting_file,
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
            commands::open_library_window,
            commands::list_whisper_models,
            commands::download_whisper_model,
            commands::get_download_progress,
            commands::cancel_download,
            commands::get_tts_status,
            commands::list_tts_voices,
            commands::list_custom_tts_models,
            commands::download_custom_tts_model,
            commands::delete_custom_tts_model,
            commands::set_active_custom_tts_model,
            commands::set_custom_tts_model_voice,
            commands::add_custom_tts_model_voice,
            commands::download_tts_voice,
            commands::download_piper_engine,
            commands::set_tts_voice,
            commands::delete_tts_voice,
            commands::open_tts_models_folder,
            commands::synthesize_speech,
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
