use crate::errors::AppError;
use dirs::{config_dir, data_dir};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
}

const APP_DIR_NAME: &str = "Ferrofluid Voice";
const SETTINGS_FILE: &str = "settings.json";

fn default_always_on() -> bool {
    true
}

fn default_hotkey_type() -> String {
    "unassigned".into()
}

fn default_auto_submit() -> bool {
    false
}

fn default_tts_voice_id() -> Option<String> {
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub model_path: Option<PathBuf>,
    pub selected_language: String,
    pub quality_profile: String,
    pub compute_backend: String,
    #[serde(default = "default_always_on")]
    pub always_on: bool,
    #[serde(default = "default_hotkey_type")]
    pub hotkey_type: String,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default = "default_tts_voice_id")]
    pub tts_voice_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model_path: None,
            selected_language: "auto".into(),
            quality_profile: "balanced".into(),
            compute_backend: "auto".into(),
            always_on: true,
            hotkey_type: "unassigned".into(),
            auto_submit: false,
            tts_voice_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub exists: bool,
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub backend: String,
    pub engine_exists: bool,
    pub engine_path: Option<String>,
    pub gpu_engine_exists: bool,
    pub gpu_engine_path: Option<String>,
}

#[derive(Debug, Clone)]
struct WhisperModelDefinition {
    id: &'static str,
    name: &'static str,
    file_name: &'static str,
    size: &'static str,
    description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub size: String,
    pub description: String,
    pub url: String,
    pub local_path: Option<String>,
    pub is_downloaded: bool,
    pub is_selected: bool,
}

const WHISPER_MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

const WHISPER_MODELS: &[WhisperModelDefinition] = &[
    WhisperModelDefinition {
        id: "tiny",
        name: "Tiny",
        file_name: "ggml-tiny.bin",
        size: "75 MB",
        description: "Fastest option for short notes and quick commands.",
    },
    WhisperModelDefinition {
        id: "base",
        name: "Base",
        file_name: "ggml-base.bin",
        size: "142 MB",
        description: "Good default for lightweight local transcription.",
    },
    WhisperModelDefinition {
        id: "small",
        name: "Small",
        file_name: "ggml-small.bin",
        size: "466 MB",
        description: "Better accuracy while still practical on most laptops.",
    },
    WhisperModelDefinition {
        id: "medium",
        name: "Medium",
        file_name: "ggml-medium.bin",
        size: "1.5 GB",
        description: "Higher quality for longer dictation and mixed speech.",
    },
    WhisperModelDefinition {
        id: "large-v3-turbo",
        name: "Large v3 Turbo",
        file_name: "ggml-large-v3-turbo.bin",
        size: "1.6 GB",
        description: "Strong quality with better speed than the full large model.",
    },
    WhisperModelDefinition {
        id: "large-v3",
        name: "Large v3",
        file_name: "ggml-large-v3.bin",
        size: "3.1 GB",
        description: "Best quality option, needs more disk and compute.",
    },
];

pub fn app_data_root() -> Result<PathBuf, AppError> {
    let base =
        data_dir().ok_or_else(|| AppError::Settings("Could not resolve data directory.".into()))?;
    Ok(base.join(APP_DIR_NAME))
}

pub fn app_config_root() -> Result<PathBuf, AppError> {
    let base = config_dir()
        .ok_or_else(|| AppError::Settings("Could not resolve config directory.".into()))?;
    Ok(base.join(APP_DIR_NAME))
}

pub fn models_dir() -> Result<PathBuf, AppError> {
    Ok(app_data_root()?.join("models"))
}

pub fn recordings_dir() -> Result<PathBuf, AppError> {
    Ok(app_data_root()?.join("recordings"))
}

pub fn tts_dir() -> Result<PathBuf, AppError> {
    Ok(app_data_root()?.join("tts"))
}

pub fn database_path() -> Result<PathBuf, AppError> {
    Ok(app_data_root()?.join("ferrofluid_voice.sqlite"))
}

pub fn ensure_app_dirs() -> Result<(), AppError> {
    fs::create_dir_all(models_dir()?)?;
    fs::create_dir_all(recordings_dir()?)?;
    fs::create_dir_all(tts_dir()?)?;
    fs::create_dir_all(app_config_root()?)?;
    Ok(())
}

pub fn load_settings() -> Result<AppSettings, AppError> {
    ensure_app_dirs()?;
    let path = app_config_root()?.join(SETTINGS_FILE);
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path)?;
    match serde_json::from_str::<AppSettings>(&contents) {
        Ok(mut settings) => {
            // Safety Healing Guard: Protect the user from lockouts if mouse_left or mouse_right was somehow saved
            if settings.hotkey_type == "mouse_left" || settings.hotkey_type == "mouse_right" {
                settings.hotkey_type = "unassigned".to_string();
                let _ = save_settings(&settings);
            }
            Ok(settings)
        }
        Err(_) => {
            // Fallback: If settings.json is corrupted or structurally incompatible,
            // initialize and save a correct default configuration to heal it.
            let default_settings = AppSettings::default();
            let _ = save_settings(&default_settings);
            Ok(default_settings)
        }
    }
}

pub fn save_settings(settings: &AppSettings) -> Result<(), AppError> {
    ensure_app_dirs()?;
    let path = app_config_root()?.join(SETTINGS_FILE);
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| AppError::Settings(error.to_string()))?;
    fs::write(path, contents)?;
    Ok(())
}

pub fn model_status(settings: &AppSettings) -> ModelStatus {
    let model_path = settings.model_path.as_ref().filter(|path| path.exists());
    let engine_candidates = whisper_binary_candidates();
    let gpu_engine_path = engine_candidates
        .iter()
        .find(|candidate| candidate.is_gpu)
        .map(|candidate| candidate.path.clone());
    let engine_path = engine_candidates
        .first()
        .map(|candidate| candidate.path.clone());
    ModelStatus {
        exists: model_path.is_some(),
        model_path: model_path.map(|path| path.to_string_lossy().to_string()),
        model_name: model_path
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_string()),
        backend: if gpu_engine_path.is_some() {
            "cuda".into()
        } else {
            settings.compute_backend.clone()
        },
        engine_exists: engine_path.is_some(),
        engine_path: engine_path.map(|path| path.to_string_lossy().to_string()),
        gpu_engine_exists: gpu_engine_path.is_some(),
        gpu_engine_path: gpu_engine_path.map(|path| path.to_string_lossy().to_string()),
    }
}

pub fn set_model_path(settings: &mut AppSettings, path: impl AsRef<Path>) -> Result<(), AppError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AppError::ModelNotFound);
    }

    settings.model_path = Some(path.to_path_buf());
    save_settings(settings)
}

pub fn available_whisper_models(settings: &AppSettings) -> Result<Vec<WhisperModelInfo>, AppError> {
    let dir = models_dir()?;
    let selected_path = settings.model_path.as_ref();

    Ok(WHISPER_MODELS
        .iter()
        .map(|model| {
            let local_path = dir.join(model.file_name);
            let is_downloaded = local_path.exists();
            let is_selected = selected_path
                .map(|path| path == &local_path)
                .unwrap_or(false);

            WhisperModelInfo {
                id: model.id.into(),
                name: model.name.into(),
                file_name: model.file_name.into(),
                size: model.size.into(),
                description: model.description.into(),
                url: model_url(model),
                local_path: is_downloaded.then(|| local_path.to_string_lossy().to_string()),
                is_downloaded,
                is_selected,
            }
        })
        .collect())
}

pub fn download_whisper_model_file(
    app: &AppHandle,
    model_id: &str,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>>>,
) -> Result<PathBuf, AppError> {
    let model = WHISPER_MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| AppError::Settings(format!("Unknown Whisper model: {model_id}")))?;

    let dir = models_dir()?;
    fs::create_dir_all(&dir)?;
    let destination = dir.join(model.file_name);
    if destination.exists() {
        return Ok(destination);
    }

    let temp_path = destination.with_extension("bin.download");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }

    let response = reqwest::blocking::Client::new()
        .get(model_url(model))
        .header(reqwest::header::USER_AGENT, "Ferrofluid Voice/0.1")
        .send()
        .map_err(|error| AppError::Download(error.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::Download(format!(
            "Could not download {}: HTTP {}",
            model.file_name,
            response.status()
        )));
    }

    let total_size = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|val| val.to_str().ok())
        .and_then(|val| val.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = response;
    let mut file = fs::File::create(&temp_path)?;
    
    let mut buffer = [0; 16384];
    let mut downloaded = 0;
    let mut last_emitted_time = std::time::Instant::now();

    // Emit initial 0% globally
    let initial_payload = DownloadProgressPayload {
        model_id: model_id.to_string(),
        downloaded_bytes: 0,
        total_bytes: total_size,
        percentage: 0.0,
    };
    let _ = app.emit("download-progress", &initial_payload);

    loop {
        // Check cancel flag
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = fs::remove_file(&temp_path);
            if let Ok(mut progress_guard) = progress_map.lock() {
                progress_guard.remove(model_id);
            }
            return Err(AppError::Download("Download cancelled by user.".into()));
        }

        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        let now = std::time::Instant::now();
        if now.duration_since(last_emitted_time).as_millis() >= 100 || downloaded == total_size {
            last_emitted_time = now;
            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            // Update thread-safe progress map
            if let Ok(mut progress_guard) = progress_map.lock() {
                progress_guard.insert(model_id.to_string(), (downloaded, total_size));
            }

            let progress_payload = DownloadProgressPayload {
                model_id: model_id.to_string(),
                downloaded_bytes: downloaded,
                total_bytes: total_size,
                percentage,
            };
            let _ = app.emit("download-progress", &progress_payload);
        }
    }

    // Update progress map with final 100% state
    if let Ok(mut progress_guard) = progress_map.lock() {
        progress_guard.insert(model_id.to_string(), (downloaded, if total_size > 0 { total_size } else { downloaded }));
    }

    // Emit final 100% progress globally when successful
    let final_payload = DownloadProgressPayload {
        model_id: model_id.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: if total_size > 0 { total_size } else { downloaded },
        percentage: 100.0,
    };
    let _ = app.emit("download-progress", &final_payload);

    fs::rename(&temp_path, &destination)?;

    Ok(destination)
}

fn model_url(model: &WhisperModelDefinition) -> String {
    format!("{WHISPER_MODEL_BASE_URL}/{}", model.file_name)
}

#[derive(Debug, Clone)]
pub struct WhisperBinaryCandidate {
    pub path: PathBuf,
    pub is_gpu: bool,
}

pub fn whisper_binary_candidates() -> Vec<WhisperBinaryCandidate> {
    if let Ok(path) = env::var("FERROFLUID_WHISPER_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return vec![WhisperBinaryCandidate {
                is_gpu: is_gpu_engine_path(&path),
                path,
            }];
        }
    }

    let cuda_executable = if cfg!(windows) {
        "whisper-cli-cuda.exe"
    } else {
        "whisper-cli-cuda"
    };
    let cpu_executable = if cfg!(windows) {
        "whisper-cli-cpu.exe"
    } else {
        "whisper-cli-cpu"
    };
    let default_executable = if cfg!(windows) {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    };

    let current_exe = env::current_exe().ok();
    let current_dir = env::current_dir().ok();
    let mut base_dirs = vec![
        current_exe
            .as_ref()
            .and_then(|path| path.parent())
            .map(|dir| dir.join("binaries")),
        current_exe
            .as_ref()
            .and_then(|path| path.parent())
            .map(|dir| dir.join("resources").join("binaries")),
        current_exe
            .as_ref()
            .and_then(|path| path.parent())
            .map(|dir| dir.to_path_buf()),
        current_dir.as_ref().map(|dir| dir.join("binaries")),
        current_dir
            .as_ref()
            .map(|dir| dir.join("src-tauri").join("binaries")),
    ];

    if let Ok(guard) = crate::commands::GLOBAL_APP_HANDLE.lock() {
        if let Some(app) = guard.as_ref() {
            use tauri::Manager;
            if let Ok(res_dir) = app.path().resource_dir() {
                base_dirs.push(Some(res_dir.join("binaries")));
            }
        }
    }

    let search_dirs: Vec<PathBuf> = base_dirs
        .iter()
        .flatten()
        .flat_map(|dir| [dir.join("cuda"), dir.join("cpu"), dir.to_path_buf()])
        .collect();

    let mut candidates = Vec::new();
    for executable in [cuda_executable, cpu_executable, default_executable] {
        for dir in search_dirs.iter() {
            let path = dir.join(executable);
            if path.exists()
                && !candidates
                    .iter()
                    .any(|candidate: &WhisperBinaryCandidate| candidate.path == path)
            {
                candidates.push(WhisperBinaryCandidate {
                    is_gpu: is_gpu_engine_path(&path),
                    path,
                });
            }
        }
    }

    candidates
}

fn is_gpu_engine_path(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase().contains("cuda"))
        .unwrap_or(false)
}
