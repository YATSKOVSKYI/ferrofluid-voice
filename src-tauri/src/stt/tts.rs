use crate::errors::AppError;
use crate::stt::model_manager::{app_data_root, save_settings, tts_dir, AppSettings, DownloadProgressPayload};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter};

const PIPER_RELEASE_URL: &str =
    "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";
const PIPER_VOICES_BASE_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

#[derive(Debug, Clone)]
struct TtsVoiceDefinition {
    id: &'static str,
    name: &'static str,
    locale: &'static str,
    quality: &'static str,
    size: &'static str,
    description: &'static str,
    repo_path: &'static str,
    file_name: &'static str,
}

const TTS_VOICES: &[TtsVoiceDefinition] = &[
    TtsVoiceDefinition {
        id: "ru_RU-dmitri-medium",
        name: "Dmitri",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian male Piper voice from rhasspy/piper-voices.",
        repo_path: "ru/ru_RU/dmitri/medium",
        file_name: "ru_RU-dmitri-medium.onnx",
    },
    TtsVoiceDefinition {
        id: "ru_RU-denis-medium",
        name: "Denis",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Alternative Russian male Piper voice from rhasspy/piper-voices.",
        repo_path: "ru/ru_RU/denis/medium",
        file_name: "ru_RU-denis-medium.onnx",
    },
    TtsVoiceDefinition {
        id: "ru_RU-irina-medium",
        name: "Irina",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian female Piper voice from rhasspy/piper-voices.",
        repo_path: "ru/ru_RU/irina/medium",
        file_name: "ru_RU-irina-medium.onnx",
    },
    TtsVoiceDefinition {
        id: "ru_RU-ruslan-medium",
        name: "Ruslan",
        locale: "ru_RU",
        quality: "Medium",
        size: "~63 MB",
        description: "Russian male Piper voice from rhasspy/piper-voices.",
        repo_path: "ru/ru_RU/ruslan/medium",
        file_name: "ru_RU-ruslan-medium.onnx",
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    pub engine_exists: bool,
    pub engine_path: Option<String>,
    pub selected_voice_id: Option<String>,
    pub selected_voice_name: Option<String>,
    pub selected_voice_downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsVoiceInfo {
    pub id: String,
    pub name: String,
    pub locale: String,
    pub quality: String,
    pub size: String,
    pub description: String,
    pub license: String,
    pub repository: String,
    pub url: String,
    pub local_model_path: Option<String>,
    pub local_config_path: Option<String>,
    pub is_downloaded: bool,
    pub is_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsSynthesisResult {
    pub audio_path: String,
    pub voice_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTtsVoiceInfo {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTtsModelInfo {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub license: String,
    pub url: String,
    pub local_path: Option<String>,
    pub companion_config_path: Option<String>,
    pub is_downloaded: bool,
    pub synthesis_supported: bool,
    pub notes: String,
    #[serde(default)]
    pub voices: Vec<CustomTtsVoiceInfo>,
    #[serde(default)]
    pub selected_voice_id: Option<String>,
    /// True when this custom model is the active app TTS voice. Computed at list
    /// time from `AppSettings::tts_voice_id`, not persisted meaningfully.
    #[serde(default)]
    pub is_selected: bool,
}

pub fn tts_models_dir() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("voices"))
}

pub fn tts_output_dir() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("output"))
}

fn custom_tts_models_dir() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("custom"))
}

fn custom_tts_manifest_path() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("custom_models.json"))
}

fn piper_engine_dir() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("engine"))
}

pub fn tts_status(settings: &AppSettings) -> TtsStatus {
    let engine_path = piper_binary_path();
    let (selected_voice_id, selected_voice_name, selected_voice_downloaded) =
        resolve_selected_label(settings);

    TtsStatus {
        engine_exists: engine_path.is_some(),
        engine_path: engine_path.map(|path| path.to_string_lossy().to_string()),
        selected_voice_id,
        selected_voice_name,
        selected_voice_downloaded,
    }
}

/// Resolves the active voice id (built-in Piper voice or custom model) into a
/// display label and "ready to synthesize" flag for the status bar.
fn resolve_selected_label(settings: &AppSettings) -> (Option<String>, Option<String>, bool) {
    let Some(voice_id) = settings.tts_voice_id.clone() else {
        return (None, None, false);
    };

    if let Some(voice) = available_tts_voices(settings)
        .ok()
        .and_then(|voices| voices.into_iter().find(|voice| voice.id == voice_id))
    {
        return (Some(voice_id), Some(voice.name), voice.is_downloaded);
    }

    if let Ok(models) = read_custom_tts_manifest() {
        if let Some(mut model) = models.into_iter().find(|model| model.id == voice_id) {
            refresh_custom_model(&mut model);
            return (Some(voice_id), Some(model.name), model.synthesis_supported);
        }
    }

    (None, None, false)
}

pub fn available_tts_voices(settings: &AppSettings) -> Result<Vec<TtsVoiceInfo>, AppError> {
    fs::create_dir_all(tts_models_dir()?)?;
    Ok(TTS_VOICES
        .iter()
        .map(|voice| {
            let model_path = voice_model_path(voice);
            let config_path = voice_config_path(voice);
            let is_downloaded = model_path.exists() && config_path.exists();
            TtsVoiceInfo {
                id: voice.id.into(),
                name: voice.name.into(),
                locale: voice.locale.into(),
                quality: voice.quality.into(),
                size: voice.size.into(),
                description: voice.description.into(),
                license: "MIT".into(),
                repository: "rhasspy/piper-voices".into(),
                url: voice_url(voice, voice.file_name),
                local_model_path: is_downloaded.then(|| model_path.to_string_lossy().to_string()),
                local_config_path: is_downloaded.then(|| config_path.to_string_lossy().to_string()),
                is_downloaded,
                is_selected: settings.tts_voice_id.as_deref() == Some(voice.id),
            }
        })
        .collect())
}

pub fn list_custom_tts_models(settings: &AppSettings) -> Result<Vec<CustomTtsModelInfo>, AppError> {
    fs::create_dir_all(custom_tts_models_dir()?)?;
    let mut models = read_custom_tts_manifest()?;
    let active = settings.tts_voice_id.as_deref();
    let mut changed = false;
    for model in &mut models {
        if refresh_custom_model(model) {
            changed = true;
        }
        model.is_selected = active == Some(model.id.as_str());
    }
    // Persist refreshed speaker lists / selections so synthesis and the UI agree.
    if changed {
        let _ = write_custom_tts_manifest(&models);
    }
    Ok(models)
}

/// What runtime, if any, can execute a custom model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CustomModelKind {
    /// `.onnx` voice runnable by the bundled Piper engine.
    PiperOnnx,
    /// `.pt` Silero model runnable by the Python sidecar.
    Silero,
    /// Nothing the app can run (missing file, missing config, unknown format).
    Unsupported,
}

fn custom_model_kind(model: &CustomTtsModelInfo) -> CustomModelKind {
    let Some(path) = model.local_path.as_ref().map(PathBuf::from) else {
        return CustomModelKind::Unsupported;
    };
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "onnx" => CustomModelKind::PiperOnnx,
        "pt" | "pth" => CustomModelKind::Silero,
        _ => CustomModelKind::Unsupported,
    }
}

/// Re-derives the runtime-dependent fields of a custom model from the files on
/// disk: whether it is downloaded, whether a runtime can run it (Piper for ONNX,
/// the Python sidecar for Silero `.pt`), the available speakers, and the status
/// note. Returns true when a stored field changed (so the manifest can re-save).
fn refresh_custom_model(model: &mut CustomTtsModelInfo) -> bool {
    let local = model.local_path.as_ref().map(PathBuf::from);
    let exists = local.as_ref().map(|path| path.exists()).unwrap_or(false);
    let kind = custom_model_kind(model);
    let config_path = model
        .companion_config_path
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.exists());
    let config_ok = config_path.is_some();
    let python_found = python_binary().is_some();

    let synthesis_supported = exists
        && match kind {
            CustomModelKind::PiperOnnx => config_ok,
            // The actual `import torch` check is deferred to selection/synthesis
            // (it is slow); here we only require that a Python interpreter exists.
            CustomModelKind::Silero => python_found,
            CustomModelKind::Unsupported => false,
        };

    let mut voices = model.voices.clone();
    let mut selected = model.selected_voice_id.clone();
    if synthesis_supported {
        voices = match kind {
            CustomModelKind::PiperOnnx => config_path
                .as_ref()
                .map(|path| piper_speakers_from_config(path))
                .unwrap_or_default(),
            CustomModelKind::Silero => local
                .as_ref()
                .map(|path| silero_speakers(path))
                .unwrap_or_default(),
            CustomModelKind::Unsupported => Vec::new(),
        };
        // Drop a stored speaker selection that no longer exists; default to the
        // first speaker when the model has multiple.
        if voices.is_empty() {
            selected = None;
        } else if selected
            .as_ref()
            .map(|id| !voices.iter().any(|voice| &voice.id == id))
            .unwrap_or(true)
        {
            selected = voices.first().map(|voice| voice.id.clone());
        }
    } else {
        voices.clear();
        selected = None;
    }

    let notes = custom_model_status_note(kind, exists, config_ok, python_found);

    let changed = model.is_downloaded != exists
        || model.synthesis_supported != synthesis_supported
        || model.voices != voices
        || model.selected_voice_id != selected
        || model.notes != notes;

    model.is_downloaded = exists;
    model.synthesis_supported = synthesis_supported;
    model.voices = voices;
    model.selected_voice_id = selected;
    model.notes = notes;
    changed
}

pub fn download_custom_tts_model_file(
    app: &AppHandle,
    url: &str,
    name: Option<String>,
    license: Option<String>,
    engine: Option<String>,
    cancel_flag: Arc<AtomicBool>,
    progress_map: Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
) -> Result<CustomTtsModelInfo, AppError> {
    let parsed_url = reqwest::Url::parse(url)
        .map_err(|error| AppError::Download(format!("Invalid model URL: {error}")))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(AppError::Download("Model URL must use http or https.".into()));
    }

    fs::create_dir_all(custom_tts_models_dir()?)?;
    let file_name = parsed_url
        .path_segments()
        .and_then(|segments| segments.last())
        .filter(|segment| !segment.trim().is_empty())
        .ok_or_else(|| AppError::Download("Model URL does not contain a file name.".into()))?;
    let safe_file_name = sanitize_file_name(file_name);
    if safe_file_name.is_empty() {
        return Err(AppError::Download("Model file name is not valid.".into()));
    }

    let display_name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| file_stem_label(&safe_file_name));
    let normalized_engine = infer_custom_engine(engine.as_deref(), &safe_file_name);
    let normalized_license = license
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "User supplied".into());
    let id = format!(
        "custom-{}-{}",
        slugify(&display_name),
        chrono::Utc::now().timestamp_millis()
    );
    let destination = custom_tts_models_dir()?.join(&id).join(&safe_file_name);
    let client = Client::new();
    let total_size = remote_content_length(&client, url).unwrap_or(0);
    let mut cumulative = 0_u64;

    download_file(
        app,
        &client,
        "custom-tts",
        url,
        &destination,
        &cancel_flag,
        &progress_map,
        total_size,
        &mut cumulative,
    )?;

    let companion_config_path = if safe_file_name.to_ascii_lowercase().ends_with(".onnx") {
        let config_url = format!("{url}.json");
        let config_destination = destination.with_file_name(format!("{safe_file_name}.json"));
        match try_download_optional_file(
            app,
            &client,
            "custom-tts",
            &config_url,
            &config_destination,
            &cancel_flag,
            &progress_map,
            &mut cumulative,
        ) {
            Ok(true) => Some(config_destination.to_string_lossy().to_string()),
            Ok(false) => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };

    let mut model = CustomTtsModelInfo {
        id,
        name: display_name,
        engine: normalized_engine,
        license: normalized_license,
        url: url.to_string(),
        local_path: Some(destination.to_string_lossy().to_string()),
        companion_config_path,
        is_downloaded: true,
        synthesis_supported: false,
        notes: String::new(),
        voices: Vec::new(),
        selected_voice_id: None,
        is_selected: false,
    };
    // Derive synthesis support, speakers and the status note from the files we
    // just wrote to disk.
    refresh_custom_model(&mut model);

    let mut models = read_custom_tts_manifest()?;
    models.retain(|existing| existing.url != model.url);
    models.push(model.clone());
    write_custom_tts_manifest(&models)?;
    Ok(model)
}

pub fn delete_custom_tts_model_file(model_id: &str) -> Result<(), AppError> {
    let mut models = read_custom_tts_manifest()?;
    let Some(index) = models.iter().position(|model| model.id == model_id) else {
        return Err(AppError::TextToSpeech(format!("Unknown custom TTS model: {model_id}")));
    };
    let model = models.remove(index);
    let mut model_dir: Option<PathBuf> = None;
    if let Some(path) = model.local_path {
        let path = PathBuf::from(path);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        if let Some(parent) = path.parent() {
            model_dir = Some(parent.to_path_buf());
        }
    }
    if let Some(path) = model.companion_config_path {
        let path = PathBuf::from(path);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    if let Some(dir) = model_dir {
        let _ = fs::remove_dir(dir);
    }
    write_custom_tts_manifest(&models)
}

pub fn set_custom_tts_model_voice_file(model_id: &str, voice_id: &str) -> Result<CustomTtsModelInfo, AppError> {
    let mut models = read_custom_tts_manifest()?;
    let Some(model) = models.iter_mut().find(|model| model.id == model_id) else {
        return Err(AppError::TextToSpeech(format!("Unknown custom TTS model: {model_id}")));
    };
    if !model.voices.iter().any(|voice| voice.id == voice_id) {
        return Err(AppError::TextToSpeech(format!("Unknown voice for this model: {voice_id}")));
    }
    model.selected_voice_id = Some(voice_id.to_string());
    let selected = model.clone();
    write_custom_tts_manifest(&models)?;
    Ok(selected)
}

pub fn add_custom_tts_model_voice_file(
    model_id: &str,
    voice_id: &str,
    name: Option<String>,
) -> Result<CustomTtsModelInfo, AppError> {
    let voice_id = voice_id.trim();
    if voice_id.is_empty() {
        return Err(AppError::TextToSpeech("Enter speaker id.".into()));
    }

    let mut models = read_custom_tts_manifest()?;
    let Some(model) = models.iter_mut().find(|model| model.id == model_id) else {
        return Err(AppError::TextToSpeech(format!("Unknown custom TTS model: {model_id}")));
    };
    if !model.voices.iter().any(|voice| voice.id == voice_id) {
        model.voices.push(CustomTtsVoiceInfo {
            id: voice_id.to_string(),
            name: name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(voice_id)
                .to_string(),
            language: "custom".into(),
            gender: None,
        });
    }
    model.selected_voice_id = Some(voice_id.to_string());
    let updated = model.clone();
    write_custom_tts_manifest(&models)?;
    Ok(updated)
}

pub fn set_tts_voice(settings: &mut AppSettings, voice_id: &str) -> Result<(), AppError> {
    let voice = voice_definition(voice_id)?;
    if !voice_model_path(voice).exists() || !voice_config_path(voice).exists() {
        return Err(AppError::TextToSpeech("Download the selected Piper voice first.".into()));
    }

    settings.tts_voice_id = Some(voice_id.to_string());
    save_settings(settings)
}

/// Marks a downloaded custom ONNX model as the active app voice so synthesis
/// uses it instead of a built-in Piper voice.
pub fn set_active_custom_tts_model_file(
    settings: &mut AppSettings,
    model_id: &str,
) -> Result<(), AppError> {
    let mut models = read_custom_tts_manifest()?;
    let Some(model) = models.iter_mut().find(|model| model.id == model_id) else {
        return Err(AppError::TextToSpeech(format!("Unknown custom TTS model: {model_id}")));
    };
    refresh_custom_model(model);
    if !model.synthesis_supported {
        return Err(AppError::TextToSpeech(format!(
            "Эту модель нельзя выбрать. {}",
            model.notes
        )));
    }
    // For Silero, confirm the Python+PyTorch runtime really works now so the
    // user sees the problem on click rather than at the first synthesis.
    if custom_model_kind(model) == CustomModelKind::Silero {
        silero_runtime().map_err(AppError::TextToSpeech)?;
    }
    // Persist the refreshed speaker list/selection before switching the active voice.
    write_custom_tts_manifest(&models)?;
    settings.tts_voice_id = Some(model_id.to_string());
    save_settings(settings)
}

pub fn delete_tts_voice(settings: &mut AppSettings, voice_id: &str) -> Result<(), AppError> {
    let voice = voice_definition(voice_id)?;
    let model_path = voice_model_path(voice);
    let config_path = voice_config_path(voice);
    if model_path.exists() {
        fs::remove_file(model_path)?;
    }
    if config_path.exists() {
        fs::remove_file(config_path)?;
    }
    if settings.tts_voice_id.as_deref() == Some(voice_id) {
        settings.tts_voice_id = None;
        save_settings(settings)?;
    }
    Ok(())
}

pub fn download_tts_voice_files(
    app: &AppHandle,
    voice_id: &str,
    cancel_flag: Arc<AtomicBool>,
    progress_map: Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
) -> Result<(), AppError> {
    let voice = voice_definition(voice_id)?;
    fs::create_dir_all(tts_models_dir()?)?;

    let downloads = vec![
        (voice.file_name.to_string(), voice_model_path(voice)),
        (format!("{}.json", voice.file_name), voice_config_path(voice)),
    ];
    let client = Client::new();
    let total_size = downloads
        .iter()
        .filter_map(|(file_name, _)| remote_content_length(&client, &voice_url(voice, file_name)))
        .sum::<u64>();
    let mut cumulative = 0_u64;

    for (file_name, destination) in downloads {
        if destination.exists() {
            cumulative += destination.metadata().map(|meta| meta.len()).unwrap_or(0);
            emit_progress(app, &progress_map, voice_id, cumulative, total_size);
            continue;
        }
        let url = voice_url(voice, &file_name);
        download_file(
            app,
            &client,
            voice_id,
            &url,
            &destination,
            &cancel_flag,
            &progress_map,
            total_size,
            &mut cumulative,
        )?;
    }

    emit_progress(app, &progress_map, voice_id, total_size.max(cumulative), total_size.max(cumulative));
    Ok(())
}

pub fn download_piper_engine(
    app: &AppHandle,
    cancel_flag: Arc<AtomicBool>,
    progress_map: Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
) -> Result<PathBuf, AppError> {
    if !cfg!(windows) {
        return Err(AppError::TextToSpeech(
            "Automatic Piper engine download is currently configured for Windows builds.".into(),
        ));
    }

    if let Some(path) = piper_binary_path() {
        return Ok(path);
    }

    let engine_dir = piper_engine_dir()?;
    fs::create_dir_all(&engine_dir)?;
    let archive_path = engine_dir.join("piper_windows_amd64.zip.download");
    let client = Client::new();
    let total_size = remote_content_length(&client, PIPER_RELEASE_URL).unwrap_or(0);
    let mut cumulative = 0_u64;
    download_file(
        app,
        &client,
        "piper-engine",
        PIPER_RELEASE_URL,
        &archive_path,
        &cancel_flag,
        &progress_map,
        total_size,
        &mut cumulative,
    )?;

    let file = fs::File::open(&archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| AppError::Download(format!("Could not read Piper archive: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::Download(format!("Could not extract Piper archive: {error}")))?;
        let Some(enclosed) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let out_path = engine_dir.join(enclosed);
        if entry.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    let _ = fs::remove_file(&archive_path);

    piper_binary_path().ok_or_else(|| {
        AppError::TextToSpeech("Piper engine was downloaded, but piper.exe was not found in the archive.".into())
    })
}

/// The concrete files and speaker the Piper engine should run, resolved from
/// either a built-in voice, a custom ONNX (Piper) model, or a custom Silero
/// `.pt` model run through the Python sidecar.
struct ResolvedTtsVoice {
    name: String,
    backend: TtsBackend,
}

enum TtsBackend {
    /// Runs through the bundled `piper.exe`.
    Piper {
        model_path: PathBuf,
        config_path: PathBuf,
        speaker: Option<i64>,
    },
    /// Runs through a system Python + PyTorch via `silero_tts.py`.
    Silero {
        model_path: PathBuf,
        speaker: Option<String>,
    },
}

fn resolve_active_voice(settings: &AppSettings) -> Result<ResolvedTtsVoice, AppError> {
    let voice_id = settings
        .tts_voice_id
        .as_deref()
        .ok_or_else(|| AppError::TextToSpeech("Выберите голос в настройках Text to Speech.".into()))?;

    // Built-in Piper voice.
    if let Some(voice) = TTS_VOICES.iter().find(|voice| voice.id == voice_id) {
        let model_path = voice_model_path(voice);
        let config_path = voice_config_path(voice);
        if !model_path.exists() || !config_path.exists() {
            return Err(AppError::TextToSpeech("Selected Piper voice is not downloaded.".into()));
        }
        return Ok(ResolvedTtsVoice {
            name: voice.name.into(),
            backend: TtsBackend::Piper {
                model_path,
                config_path,
                speaker: None,
            },
        });
    }

    // Custom model added by the user via a link.
    let models = read_custom_tts_manifest()?;
    let mut model = models
        .into_iter()
        .find(|model| model.id == voice_id)
        .ok_or_else(|| AppError::TextToSpeech(format!("Unknown TTS voice: {voice_id}")))?;
    refresh_custom_model(&mut model);
    if !model.synthesis_supported {
        return Err(AppError::TextToSpeech(format!(
            "Модель «{}» нельзя синтезировать. {}",
            model.name, model.notes
        )));
    }
    let model_path = model
        .local_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::TextToSpeech("Файл выбранной модели не найден. Скачайте её заново.".into()))?;

    match custom_model_kind(&model) {
        CustomModelKind::Silero => Ok(ResolvedTtsVoice {
            name: model.name.clone(),
            backend: TtsBackend::Silero {
                model_path,
                speaker: model.selected_voice_id.clone(),
            },
        }),
        CustomModelKind::PiperOnnx => {
            let config_path = model
                .companion_config_path
                .as_ref()
                .map(PathBuf::from)
                .ok_or_else(|| AppError::TextToSpeech("Для выбранной модели отсутствует config (.onnx.json).".into()))?;
            let speaker = model
                .selected_voice_id
                .as_deref()
                .and_then(|id| id.parse::<i64>().ok());
            Ok(ResolvedTtsVoice {
                name: model.name.clone(),
                backend: TtsBackend::Piper {
                    model_path,
                    config_path,
                    speaker,
                },
            })
        }
        CustomModelKind::Unsupported => Err(AppError::TextToSpeech(format!(
            "Формат модели «{}» не поддерживается.",
            model.name
        ))),
    }
}

pub fn synthesize_speech(settings: AppSettings, text: String) -> Result<TtsSynthesisResult, AppError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::TextToSpeech("Enter text to synthesize.".into()));
    }

    let voice = resolve_active_voice(&settings)?;

    fs::create_dir_all(tts_output_dir()?)?;
    let audio_path = tts_output_dir()?.join(format!(
        "ferrofluid-tts-{}.wav",
        chrono::Utc::now().timestamp_millis()
    ));

    match &voice.backend {
        TtsBackend::Piper {
            model_path,
            config_path,
            speaker,
        } => synthesize_with_piper(model_path, config_path, *speaker, text, &audio_path)?,
        TtsBackend::Silero {
            model_path,
            speaker,
        } => synthesize_with_silero(model_path, speaker.as_deref(), text, &audio_path)?,
    }

    if !audio_path.exists() {
        return Err(AppError::TextToSpeech("TTS-движок не создал аудиофайл.".into()));
    }

    Ok(TtsSynthesisResult {
        audio_path: audio_path.to_string_lossy().to_string(),
        voice_name: voice.name,
    })
}

fn synthesize_with_piper(
    model_path: &Path,
    config_path: &Path,
    speaker: Option<i64>,
    text: &str,
    audio_path: &Path,
) -> Result<(), AppError> {
    let piper_path = piper_binary_path()
        .ok_or_else(|| AppError::TextToSpeech("Piper engine is missing. Download it in Text to Speech settings.".into()))?;

    let mut command = Command::new(&piper_path);
    if let Some(parent) = piper_path.parent() {
        command.current_dir(parent);
        command.arg("--espeak_data").arg(parent.join("espeak-ng-data"));
    }
    command
        .arg("--model")
        .arg(model_path)
        .arg("--config")
        .arg(config_path)
        .arg("--output_file")
        .arg(audio_path);
    if let Some(speaker) = speaker {
        command.arg("--speaker").arg(speaker.to_string());
    }

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::TextToSpeech(format!("Could not start Piper: {error}")))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| AppError::TextToSpeech(format!("Could not write text to Piper: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| AppError::TextToSpeech(format!("Piper failed: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::TextToSpeech(format!(
            "Piper exited with status {}. {}",
            output.status,
            stderr.trim()
        )));
    }
    Ok(())
}

fn synthesize_with_silero(
    model_path: &Path,
    speaker: Option<&str>,
    text: &str,
    audio_path: &Path,
) -> Result<(), AppError> {
    let python = silero_runtime().map_err(AppError::TextToSpeech)?;
    let script = silero_script_path()?;

    let request = serde_json::json!({
        "mode": "synthesize",
        "model": model_path.to_string_lossy(),
        "output": audio_path.to_string_lossy(),
        "text": text,
        "speaker": speaker,
        "sampleRate": 48000,
    });

    let mut child = Command::new(&python)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::TextToSpeech(format!("Не удалось запустить Python: {error}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request.to_string().as_bytes())
            .map_err(|error| AppError::TextToSpeech(format!("Не удалось передать данные в Python: {error}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| AppError::TextToSpeech(format!("Silero sidecar failed: {error}")))?;

    if !output.status.success() {
        let message = silero_error_from_output(&output.stdout)
            .unwrap_or_else(|| String::from_utf8_lossy(&output.stderr).trim().to_string());
        return Err(AppError::TextToSpeech(format!("Silero: {message}")));
    }
    Ok(())
}

fn silero_error_from_output(stdout: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    value
        .get("error")
        .and_then(|error| error.as_str())
        .map(str::to_string)
}

pub fn open_tts_folder() -> Result<(), AppError> {
    let dir = tts_dir()?;
    fs::create_dir_all(&dir)?;
    opener::open(dir).map_err(|error| AppError::File(error.to_string()))
}

fn voice_definition(voice_id: &str) -> Result<&'static TtsVoiceDefinition, AppError> {
    TTS_VOICES
        .iter()
        .find(|voice| voice.id == voice_id)
        .ok_or_else(|| AppError::TextToSpeech(format!("Unknown Piper voice: {voice_id}")))
}

fn voice_dir(voice: &TtsVoiceDefinition) -> PathBuf {
    tts_models_dir()
        .unwrap_or_else(|_| PathBuf::from("tts").join("voices"))
        .join(voice.id)
}

fn voice_model_path(voice: &TtsVoiceDefinition) -> PathBuf {
    voice_dir(voice).join(voice.file_name)
}

fn voice_config_path(voice: &TtsVoiceDefinition) -> PathBuf {
    voice_dir(voice).join(format!("{}.json", voice.file_name))
}

fn voice_url(voice: &TtsVoiceDefinition, file_name: &str) -> String {
    format!("{PIPER_VOICES_BASE_URL}/{}/{}", voice.repo_path, file_name)
}

fn remote_content_length(client: &Client, url: &str) -> Option<u64> {
    client
        .head(url)
        .header(reqwest::header::USER_AGENT, "Ferrofluid Voice/1.0")
        .send()
        .ok()
        .and_then(|response| {
            response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
        })
}

fn download_file(
    app: &AppHandle,
    client: &Client,
    progress_id: &str,
    url: &str,
    destination: &Path,
    cancel_flag: &AtomicBool,
    progress_map: &Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
    total_size: u64,
    cumulative: &mut u64,
) -> Result<(), AppError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = destination.with_extension("download");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }

    let mut response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Ferrofluid Voice/1.0")
        .send()
        .map_err(|error| AppError::Download(error.to_string()))?;
    if !response.status().is_success() {
        return Err(AppError::Download(format!("Could not download {url}: HTTP {}", response.status())));
    }

    let fallback_total = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let resolved_total = total_size.max(fallback_total);

    let mut file = fs::File::create(&temp_path)?;
    let mut buffer = [0; 16384];
    let mut last_emit = std::time::Instant::now();
    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            let _ = fs::remove_file(&temp_path);
            if let Ok(mut guard) = progress_map.lock() {
                guard.remove(progress_id);
            }
            return Err(AppError::Download("Download cancelled by user.".into()));
        }

        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])?;
        *cumulative += bytes_read as u64;
        if last_emit.elapsed().as_millis() >= 100 {
            last_emit = std::time::Instant::now();
            emit_progress(app, progress_map, progress_id, *cumulative, resolved_total);
        }
    }

    fs::rename(temp_path, destination)?;
    emit_progress(app, progress_map, progress_id, *cumulative, resolved_total);
    Ok(())
}

fn try_download_optional_file(
    app: &AppHandle,
    client: &Client,
    progress_id: &str,
    url: &str,
    destination: &Path,
    cancel_flag: &AtomicBool,
    progress_map: &Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
    cumulative: &mut u64,
) -> Result<bool, AppError> {
    let response = client
        .head(url)
        .header(reqwest::header::USER_AGENT, "Ferrofluid Voice/1.0")
        .send()
        .map_err(|error| AppError::Download(error.to_string()))?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let total_size = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    download_file(
        app,
        client,
        progress_id,
        url,
        destination,
        cancel_flag,
        progress_map,
        total_size,
        cumulative,
    )?;
    Ok(true)
}

fn read_custom_tts_manifest() -> Result<Vec<CustomTtsModelInfo>, AppError> {
    let path = custom_tts_manifest_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)?;
    serde_json::from_str(&contents)
        .map_err(|error| AppError::TextToSpeech(format!("Could not read custom TTS models: {error}")))
}

fn write_custom_tts_manifest(models: &[CustomTtsModelInfo]) -> Result<(), AppError> {
    let path = custom_tts_manifest_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(models)
        .map_err(|error| AppError::TextToSpeech(format!("Could not save custom TTS models: {error}")))?;
    fs::write(path, contents)?;
    Ok(())
}

fn infer_custom_engine(engine: Option<&str>, file_name: &str) -> String {
    let explicit = engine.map(str::trim).unwrap_or("");
    if !explicit.is_empty() && explicit != "auto" {
        return explicit.to_string();
    }
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".pt") {
        "PyTorch / Silero-compatible".into()
    } else if lower.ends_with(".jit") || lower.ends_with(".model") {
        "TorchScript".into()
    } else if lower.ends_with(".onnx") {
        "ONNX / Piper-compatible".into()
    } else {
        "Custom TTS model".into()
    }
}

/// Status note shown under a custom model, tailored to the runtime that would
/// run it (Piper for ONNX, the Python sidecar for Silero `.pt`).
fn custom_model_status_note(
    kind: CustomModelKind,
    exists: bool,
    config_ok: bool,
    python_found: bool,
) -> String {
    if !exists {
        return "Файл модели не найден. Скачайте модель заново.".into();
    }
    match kind {
        CustomModelKind::PiperOnnx => {
            if config_ok {
                "Готово к синтезу (движок Piper). Нажмите «Выбрать», чтобы сделать модель активным голосом.".into()
            } else {
                "Рядом с .onnx нужен файл конфигурации Piper (.onnx.json). Добавьте его в папку модели."
                    .into()
            }
        }
        CustomModelKind::Silero => {
            if python_found {
                "Модель Silero (.pt). Озвучка идёт через системный Python + PyTorch. \
Нажмите «Выбрать», чтобы сделать её активным голосом. Если синтез выдаст ошибку про torch — выполните `pip install torch`."
                    .into()
            } else {
                "Модель Silero (.pt) требует Python с PyTorch, но Python не найден в системе. \
Установите Python и выполните `pip install torch`, затем перезапустите приложение."
                    .into()
            }
        }
        CustomModelKind::Unsupported => {
            "Формат не поддерживается. Нужна ONNX/Piper-модель (.onnx) или Silero (.pt)."
                .into()
        }
    }
}

/// Known multi-speaker voices for Russian Silero models (v3/v4/v5 share this
/// set). Used to populate the speaker dropdown without loading the 100+ MB
/// model just to read its speaker list.
fn silero_speakers(model_path: &Path) -> Vec<CustomTtsVoiceInfo> {
    let lower = model_path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let is_russian = lower.contains("_ru") || lower.contains("ru_") || lower.contains("ru.");
    if !is_russian {
        return Vec::new();
    }
    [
        ("aidar", "Aidar", "male"),
        ("baya", "Baya", "female"),
        ("kseniya", "Kseniya", "female"),
        ("xenia", "Xenia", "female"),
        ("eugene", "Eugene", "male"),
        ("random", "Random", "neutral"),
    ]
    .iter()
    .map(|(id, name, gender)| CustomTtsVoiceInfo {
        id: (*id).to_string(),
        name: (*name).to_string(),
        language: "ru".into(),
        gender: Some((*gender).to_string()),
    })
    .collect()
}

/// Locates a Python interpreter for the Silero sidecar. Honors the
/// `FERROFLUID_PYTHON_BIN` override, then falls back to PATH.
fn python_binary() -> Option<PathBuf> {
    if let Ok(path) = env::var("FERROFLUID_PYTHON_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let candidates: &[&str] = if cfg!(windows) {
        &["python.exe", "python3.exe", "py.exe"]
    } else {
        &["python3", "python"]
    };
    for candidate in candidates {
        if let Some(path) = find_on_path(candidate) {
            return Some(path);
        }
    }
    None
}

/// Verifies (once per process) that a Python with PyTorch is available. The
/// `import torch` probe is slow, so the result is cached; the user must restart
/// the app after installing torch for it to be re-detected.
fn silero_runtime() -> Result<PathBuf, String> {
    static RUNTIME: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            let python = python_binary().ok_or_else(|| {
                "Python не найден в системе. Установите Python и выполните `pip install torch`, затем перезапустите приложение."
                    .to_string()
            })?;
            let output = Command::new(&python)
                .arg("-c")
                .arg("import torch")
                .output()
                .map_err(|error| format!("Не удалось запустить Python ({}): {error}", python.display()))?;
            if output.status.success() {
                Ok(python)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!(
                    "В Python ({}) нет PyTorch. Выполните `pip install torch` и перезапустите приложение. {}",
                    python.display(),
                    stderr.trim()
                ))
            }
        })
        .clone()
}

/// Materializes the embedded Silero sidecar script next to the TTS data so it
/// can be passed to Python. Rewrites the file when the bundled script changes.
fn silero_script_path() -> Result<PathBuf, AppError> {
    const SILERO_SCRIPT: &str = include_str!("silero_tts.py");
    let dir = tts_dir()?.join("runtime");
    fs::create_dir_all(&dir)?;
    let path = dir.join("silero_tts.py");
    let needs_write = match fs::read_to_string(&path) {
        Ok(existing) => existing != SILERO_SCRIPT,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&path, SILERO_SCRIPT)?;
    }
    Ok(path)
}

/// Reads the speaker table from a Piper voice config (`*.onnx.json`). Returns an
/// empty list for single-speaker models (no `--speaker` argument needed).
fn piper_speakers_from_config(config_path: &Path) -> Vec<CustomTtsVoiceInfo> {
    let Ok(contents) = fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return Vec::new();
    };
    let num_speakers = json
        .get("num_speakers")
        .and_then(|value| value.as_u64())
        .unwrap_or(1);
    if num_speakers <= 1 {
        return Vec::new();
    }

    let mut voices: Vec<CustomTtsVoiceInfo> = Vec::new();
    if let Some(map) = json.get("speaker_id_map").and_then(|value| value.as_object()) {
        for (name, index) in map {
            if let Some(id) = index.as_i64() {
                voices.push(CustomTtsVoiceInfo {
                    id: id.to_string(),
                    name: name.clone(),
                    language: String::new(),
                    gender: None,
                });
            }
        }
        voices.sort_by_key(|voice| voice.id.parse::<i64>().unwrap_or(i64::MAX));
    }
    if voices.is_empty() {
        voices = (0..num_speakers)
            .map(|index| CustomTtsVoiceInfo {
                id: index.to_string(),
                name: format!("Speaker {index}"),
                language: String::new(),
                gender: None,
            })
            .collect();
    }
    voices
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn file_stem_label(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(['_', '-'], " "))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Custom TTS model".into())
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "tts-model".into()
    } else {
        slug
    }
}

fn emit_progress(
    app: &AppHandle,
    progress_map: &Arc<Mutex<std::collections::HashMap<String, (u64, u64)>>>,
    progress_id: &str,
    downloaded: u64,
    total: u64,
) {
    if let Ok(mut guard) = progress_map.lock() {
        guard.insert(progress_id.to_string(), (downloaded, total));
    }
    let percentage = if total > 0 {
        (downloaded as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    let payload = DownloadProgressPayload {
        model_id: progress_id.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total,
        percentage: percentage.min(100.0),
    };
    let _ = app.emit("download-progress", &payload);
}

fn piper_binary_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("FERROFLUID_PIPER_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let executable = if cfg!(windows) { "piper.exe" } else { "piper" };
    let mut roots = Vec::new();
    if let Ok(path) = app_data_root() {
        roots.push(path.join("tts").join("engine"));
    }
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            roots.push(parent.join("binaries"));
            roots.push(parent.join("resources").join("binaries"));
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        roots.push(current_dir.join("binaries"));
        roots.push(current_dir.join("src-tauri").join("binaries"));
    }
    if let Ok(guard) = crate::commands::GLOBAL_APP_HANDLE.lock() {
        if let Some(app) = guard.as_ref() {
            use tauri::Manager;
            if let Ok(res_dir) = app.path().resource_dir() {
                roots.push(res_dir.join("binaries"));
            }
        }
    }

    for root in roots {
        for candidate in [
            root.join(executable),
            root.join("piper").join(executable),
            root.join("piper_windows_amd64").join(executable),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if let Some(found) = find_file_recursive(&root, executable, 4) {
            return Some(found);
        }
    }

    find_on_path(executable)
}

fn find_file_recursive(root: &Path, file_name: &str, depth: usize) -> Option<PathBuf> {
    if depth == 0 || !root.exists() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .map(|name| name.to_string_lossy().eq_ignore_ascii_case(file_name))
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, file_name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn find_on_path(executable: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(executable);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
