use crate::{
    errors::AppError,
    stt::model_manager::{whisper_binary_candidates, AppSettings, WhisperBinaryCandidate},
};
use serde::Serialize;
use std::{
    env, fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResult {
    pub text: String,
    pub language: String,
    pub duration_seconds: f64,
    pub model_name: String,
}

pub fn transcribe(
    settings: AppSettings,
    audio_path: impl AsRef<Path>,
    language: String,
    duration_seconds: f64,
) -> Result<TranscriptResult, AppError> {
    let model_path = settings
        .model_path
        .as_ref()
        .filter(|path| path.exists())
        .ok_or(AppError::ModelNotFound)?;

    let whisper_bins = whisper_binary_candidates();
    if whisper_bins.is_empty() {
        return Err(AppError::WhisperBinaryNotFound);
    }
    let output_base = env::temp_dir().join(format!(
        "voiceglass-transcript-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| AppError::Transcription(error.to_string()))?
            .as_millis()
    ));

    let mut last_error = None;
    for whisper_bin in whisper_bins {
        match run_whisper(
            &whisper_bin,
            model_path,
            audio_path.as_ref(),
            &language,
            &output_base,
        ) {
            Ok(text) => {
                return Ok(TranscriptResult {
                    text,
                    language: language_for_result(language),
                    duration_seconds,
                    model_name: model_path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".into()),
                });
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or(AppError::WhisperBinaryNotFound))
}

fn run_whisper(
    whisper_bin: &WhisperBinaryCandidate,
    model_path: &Path,
    audio_path: &Path,
    language: &str,
    output_base: &Path,
) -> Result<String, AppError> {
    let whisper_language = match language {
        "ru" => "ru",
        "en" => "en",
        _ => "auto",
    };

    let output = Command::new(&whisper_bin.path)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(audio_path)
        .arg("-l")
        .arg(whisper_language)
        .arg("-otxt")
        .arg("-of")
        .arg(&output_base)
        .arg("-nt")
        .output()
        .map_err(|error| AppError::Transcription(error.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let backend = if whisper_bin.is_gpu { "CUDA" } else { "CPU" };
        return Err(AppError::Transcription(if stderr.is_empty() {
            stdout
        } else {
            format!("{backend} engine failed: {stderr}")
        }));
    }

    let transcript_path = output_base.with_extension("txt");
    let text = fs::read_to_string(&transcript_path)
        .map_err(|error| AppError::Transcription(error.to_string()))?
        .trim()
        .to_string();
    let _ = fs::remove_file(transcript_path);

    Ok(text)
}

fn language_for_result(language: String) -> String {
    match language.as_str() {
        "ru" | "en" | "auto" => language,
        _ => "unknown".into(),
    }
}
