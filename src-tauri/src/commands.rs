use crate::{
    audio::recorder::{AudioCaptureInfo, AudioRecorder},
    errors::AppError,
    storage::db::{Database, HistoryItem, SaveTranscriptInput},
    stt::{
        model_manager::{
            available_whisper_models, database_path, download_whisper_model_file, ensure_app_dirs,
            load_settings, model_status, models_dir, recordings_dir, save_settings,
            set_model_path as persist_model_path, AppSettings, ModelStatus, WhisperModelInfo,
        },
        whisper::{transcribe, TranscriptResult},
    },
    system::{clipboard, paths, hotkey, keyboard},
};
use chrono::Utc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub type HWND = *mut std::ffi::c_void;
pub type HHOOK = *mut std::ffi::c_void;

pub static mut H_HOOK_KEYBOARD: HHOOK = std::ptr::null_mut();
pub static mut H_HOOK_MOUSE: HHOOK = std::ptr::null_mut();
pub static mut PREV_FOREGROUND_WINDOW: HWND = std::ptr::null_mut();
pub static mut HOOK_THREAD_ID: u32 = 0;
pub static IS_RECORDING: AtomicBool = AtomicBool::new(false);
pub static IS_RECORDING_HOTKEY: AtomicBool = AtomicBool::new(false);
pub static GLOBAL_APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn SetForegroundWindow(hwnd: HWND) -> i32;
}

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub recorder: Mutex<Option<AudioRecorder>>,
    pub last_audio: Mutex<Option<AudioCaptureInfo>>,
    pub db: Mutex<Database>,
    pub download_cancel: Arc<AtomicBool>,
    pub download_progress: Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self, AppError> {
        ensure_app_dirs()?;
        let settings = load_settings()?;
        let db = Database::open(database_path()?)?;
        let _ = app.path().app_data_dir();

        Ok(Self {
            settings: Mutex::new(settings),
            recorder: Mutex::new(None),
            last_audio: Mutex::new(None),
            db: Mutex::new(db),
            download_cancel: Arc::new(AtomicBool::new(false)),
            download_progress: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }
}

#[tauri::command]
pub fn start_recording(state: tauri::State<AppState>) -> Result<(), AppError> {
    start_recording_internal(&state)
}

pub fn start_recording_internal(state: &AppState) -> Result<(), AppError> {
    let mut recorder_guard = state
        .recorder
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock was poisoned.".into()))?;
    if recorder_guard.is_some() {
        return Err(AppError::RecordingAlreadyRunning);
    }

    let path = recordings_dir()?.join(format!("ferrofluid-voice-{}.wav", Utc::now().timestamp_millis()));
    paths::ensure_parent(&path)?;
    let recorder = AudioRecorder::start(path)?;
    *recorder_guard = Some(recorder);
    IS_RECORDING.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: tauri::State<AppState>) -> Result<AudioCaptureInfo, AppError> {
    stop_recording_internal(&state)
}

pub fn stop_recording_internal(state: &AppState) -> Result<AudioCaptureInfo, AppError> {
    let recorder = state
        .recorder
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock was poisoned.".into()))?
        .take()
        .ok_or(AppError::RecordingNotRunning)?;

    IS_RECORDING.store(false, Ordering::SeqCst);

    let info = recorder.stop()?;
    *state
        .last_audio
        .lock()
        .map_err(|_| AppError::Audio("Last audio lock was poisoned.".into()))? = Some(info.clone());
    Ok(info)
}

#[tauri::command]
pub fn get_recording_state(state: tauri::State<AppState>) -> Result<bool, AppError> {
    let recorder_guard = state
        .recorder
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock was poisoned.".into()))?;
    Ok(recorder_guard.is_some())
}

#[tauri::command]
pub async fn transcribe_audio(
    state: tauri::State<'_, AppState>,
    language: String,
) -> Result<TranscriptResult, AppError> {
    let audio = state
        .last_audio
        .lock()
        .map_err(|_| AppError::Audio("Last audio lock was poisoned.".into()))?
        .clone()
        .ok_or_else(|| AppError::Audio("No recorded audio is available.".into()))?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?
        .clone();

    tauri::async_runtime::spawn_blocking(move || {
        transcribe(settings, audio.path, language, audio.duration_seconds)
    })
    .await
    .map_err(|error| AppError::Transcription(error.to_string()))?
}

#[tauri::command]
pub fn get_model_status(state: tauri::State<AppState>) -> Result<ModelStatus, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    Ok(model_status(&settings))
}

#[tauri::command]
pub fn set_model_path(
    app: AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<ModelStatus, AppError> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    persist_model_path(&mut settings, path)?;
    let status = model_status(&settings);
    let _ = app.emit("model-status-changed", &status);
    Ok(status)
}

#[tauri::command]
pub fn open_models_folder() -> Result<(), AppError> {
    let dir = models_dir()?;
    std::fs::create_dir_all(&dir)?;
    opener::open(dir).map_err(|error| AppError::File(error.to_string()))
}

#[tauri::command]
pub fn start_window_drag(window: WebviewWindow) -> Result<(), AppError> {
    window
        .start_dragging()
        .map_err(|error| AppError::File(error.to_string()))
}

#[tauri::command]
pub fn close_current_window(window: WebviewWindow) -> Result<(), AppError> {
    window
        .close()
        .map_err(|error| AppError::File(error.to_string()))
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        window
            .set_focus()
            .map_err(|error| AppError::File(error.to_string()))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("Ferrofluid Voice Settings")
    .inner_size(1040.0, 760.0)
    .min_inner_size(860.0, 620.0)
    .resizable(true)
    .decorations(true)
    .transparent(false)
    .always_on_top(false)
    .center()
    .build()
    .map_err(|error| AppError::File(error.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn list_whisper_models(
    state: tauri::State<AppState>,
) -> Result<Vec<WhisperModelInfo>, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    available_whisper_models(&settings)
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<ModelStatus, AppError> {
    // Reset cancel flag at the beginning of the download
    state.download_cancel.store(false, Ordering::SeqCst);
    {
        let mut progress_guard = state.download_progress.lock().map_err(|_| AppError::Download("Progress lock poisoned".into()))?;
        progress_guard.insert(model_id.clone(), (0, 0));
    }

    let app_clone = app.clone();
    let model_id_clone = model_id.clone();
    let cancel_flag = state.download_cancel.clone();
    let progress_map = state.download_progress.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        download_whisper_model_file(&app_clone, &model_id_clone, cancel_flag, progress_map)
    })
    .await
    .map_err(|error| AppError::Download(error.to_string()))??;

    // Clean up progress map after successful download finishes
    {
        let mut progress_guard = state.download_progress.lock().map_err(|_| AppError::Download("Progress lock poisoned".into()))?;
        progress_guard.remove(&model_id);
    }

    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    persist_model_path(&mut settings, path)?;
    let status = model_status(&settings);
    let _ = app.emit("model-status-changed", &status);
    Ok(status)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressInfo {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
}

#[tauri::command]
pub fn get_download_progress(
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<Option<DownloadProgressInfo>, AppError> {
    let progress_guard = state
        .download_progress
        .lock()
        .map_err(|_| AppError::Download("Progress lock poisoned".into()))?;
    
    if let Some(&(downloaded, total)) = progress_guard.get(&model_id) {
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        Ok(Some(DownloadProgressInfo {
            downloaded_bytes: downloaded,
            total_bytes: total,
            percentage,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn cancel_download(state: tauri::State<'_, AppState>) {
    state.download_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn delete_whisper_model(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<ModelStatus, AppError> {
    // 1. Get the model list and find the target model details
    let settings_guard = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock poisoned".into()))?;
    let whisper_models = available_whisper_models(&settings_guard)?;
    drop(settings_guard);

    let model = whisper_models.iter().find(|m| m.id == model_id)
        .ok_or_else(|| AppError::Settings(format!("Unknown Whisper model: {model_id}")))?;

    // 2. Remove file if exists
    if let Some(ref path_str) = model.local_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }

    // 3. Clear settings model_path if this was the active model
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock poisoned".into()))?;
    if let Some(ref active_path) = settings.model_path {
        if let Some(ref path_str) = model.local_path {
            if active_path == &PathBuf::from(path_str) {
                settings.model_path = None;
                save_settings(&settings)?;
            }
        }
    }

    let status = model_status(&settings);
    let _ = app.emit("model-status-changed", &status);
    Ok(status)
}

#[tauri::command]
pub fn save_transcript(
    state: tauri::State<AppState>,
    text: String,
    language: String,
    duration: f64,
    model_name: String,
) -> Result<i64, AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Storage("Database lock was poisoned.".into()))?;
    db.save_transcript(SaveTranscriptInput {
        text,
        language,
        duration,
        model_name,
    })
}

#[tauri::command]
pub fn get_history(state: tauri::State<AppState>) -> Result<Vec<HistoryItem>, AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Storage("Database lock was poisoned.".into()))?;
    db.history()
}

#[tauri::command]
pub fn delete_history_item(state: tauri::State<AppState>, id: i64) -> Result<(), AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Storage("Database lock was poisoned.".into()))?;
    db.delete_history_item(id)
}

#[tauri::command]
pub fn export_txt(text: String) -> Result<(), AppError> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export transcript")
        .set_file_name("transcript.txt")
        .add_filter("Text", &["txt"])
        .save_file()
    else {
        return Ok(());
    };

    std::fs::write(path, text)?;
    Ok(())
}

#[tauri::command]
pub fn write_clipboard(text: String) -> Result<(), AppError> {
    clipboard::write_text(text)
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    pub always_on: bool,
    pub hotkey_type: String,
    pub hotkey_display: String,
    pub auto_submit: bool,
}

fn safe_hotkey_type(hotkey_type: String) -> String {
    match hotkey_type.as_str() {
        "mouse_left" | "mouse_right" | "" => "mouse_middle".to_string(),
        _ => hotkey_type,
    }
}

#[tauri::command]
pub fn get_hotkey_settings(state: tauri::State<AppState>) -> Result<HotkeySettings, AppError> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    let safe_type = safe_hotkey_type(settings.hotkey_type.clone());
    if safe_type != settings.hotkey_type {
        settings.hotkey_type = safe_type;
        save_settings(&settings)?;
    }

    Ok(HotkeySettings {
        always_on: settings.always_on,
        hotkey_type: settings.hotkey_type.clone(),
        hotkey_display: hotkey::parse_hotkey_display(&settings.hotkey_type),
        auto_submit: settings.auto_submit,
    })
}

#[tauri::command]
pub fn start_recording_hotkey() -> Result<(), AppError> {
    IS_RECORDING_HOTKEY.store(true, std::sync::atomic::Ordering::SeqCst);
    println!("START RECORDING HOTKEY CALLED: IS_RECORDING_HOTKEY=true");
    Ok(())
}

#[tauri::command]
pub fn cancel_recording_hotkey() -> Result<(), AppError> {
    IS_RECORDING_HOTKEY.store(false, std::sync::atomic::Ordering::SeqCst);
    println!("CANCEL RECORDING HOTKEY CALLED: IS_RECORDING_HOTKEY=false");
    Ok(())
}

#[tauri::command]
pub fn update_hotkey_settings(
    app: AppHandle,
    state: tauri::State<AppState>,
    always_on: bool,
    hotkey_type: String,
    auto_submit: bool,
) -> Result<(), AppError> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Settings("Settings lock was poisoned.".into()))?;
    settings.always_on = always_on;
    settings.hotkey_type = safe_hotkey_type(hotkey_type);
    settings.auto_submit = auto_submit;
    save_settings(&settings)?;

    // Emit event for real-time synchronization across windows
    let _ = app.emit("hotkey-settings-changed", HotkeySettings {
        always_on,
        hotkey_type: settings.hotkey_type.clone(),
        hotkey_display: hotkey::parse_hotkey_display(&settings.hotkey_type),
        auto_submit,
    });

    // Dynamic Widget window visibility toggle
    if let Some(main_window) = app.get_webview_window("main") {
        if always_on {
            let _ = main_window.show();
            let _ = main_window.unminimize();
            let _ = main_window.set_focus();
        } else {
            let _ = main_window.hide();
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn inject_text(text: String, auto_submit: bool) -> Result<(), AppError> {
    // 1. Focus the previously active foreground window
    unsafe {
        if !PREV_FOREGROUND_WINDOW.is_null() {
            #[cfg(target_os = "windows")]
            SetForegroundWindow(PREV_FOREGROUND_WINDOW);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to set visible of process \"Ferrofluid Voice\" to false")
            .status();
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to set visible of process \"voiceglass\" to false")
            .status();
    }

    // 2. Wait for focus transitions to settle
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // 3. Set text to clipboard
    clipboard::write_text(text)?;

    // 4. Simulate paste shortcut (Ctrl+V)
    keyboard::simulate_paste();

    // 5. If auto-submit is requested, wait and simulate Enter keypress
    if auto_submit {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        keyboard::simulate_enter();
    }

    Ok(())
}

pub fn start_hook_thread() {
    hotkey::start_hook_thread();
}

pub fn stop_hook_thread() {
    hotkey::stop_hook_thread();
}

#[tauri::command]
pub fn log_message(message: String) {
    println!("[FRONTEND LOG] {}", message);
}
