use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Microphone is not available.")]
    NoMicrophone,
    #[error("Recording is already running.")]
    RecordingAlreadyRunning,
    #[error("Recording is not running.")]
    RecordingNotRunning,
    #[error("Microphone capture failed: {0}")]
    Audio(String),
    #[error("Whisper model was not found. Choose a local .bin model first.")]
    ModelNotFound,
    #[error("Whisper executable was not found. Set FERROFLUID_WHISPER_BIN or place whisper-cli in the app binaries folder.")]
    WhisperBinaryNotFound,
    #[error("Transcription failed: {0}")]
    Transcription(String),
    #[error("Settings error: {0}")]
    Settings(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("File error: {0}")]
    File(String),
    #[error("Download error: {0}")]
    Download(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub message: String,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ErrorPayload {
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::File(value.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Storage(value.to_string())
    }
}
