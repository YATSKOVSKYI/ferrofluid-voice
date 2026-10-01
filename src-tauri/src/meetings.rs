use crate::{
    commands::AppState,
    errors::AppError,
    stt::model_manager::{app_data_root, whisper_binary_candidates},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct MeetingsState {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<serde_json::Value>>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
    pub id: String,
    pub name: String,
    pub sample_start: f64,
    pub sample_end: f64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub start: f64,
    pub end: f64,
    pub speaker_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub speaker_id: Option<String>,
    pub text: String,
    pub needs_review: bool,
    #[serde(default)]
    pub speaker_locked: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub source_name: String,
    pub audio_path: String,
    pub duration_seconds: f64,
    pub created_at: String,
    pub language: String,
    pub model_name: Option<String>,
    pub speakers: Vec<Speaker>,
    pub turns: Vec<Turn>,
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub calibration: Option<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceReference {
    pub speaker_id: String,
    pub name: String,
    pub start: f64,
    pub end: f64,
}
#[derive(Deserialize)]
struct CalibrationResult {
    turns: Vec<Turn>,
    segments: Vec<Segment>,
    calibration: serde_json::Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsStatus {
    python_ready: bool,
    ffmpeg_ready: bool,
    diarization_ready: bool,
    busy: bool,
    progress: Option<serde_json::Value>,
}

fn root() -> Result<PathBuf, AppError> {
    Ok(app_data_root()?.join("meetings"))
}
fn engine_root() -> Result<PathBuf, AppError> {
    Ok(root()?.join("engine"))
}
fn directory(id: &str) -> Result<PathBuf, AppError> {
    if !id.starts_with("meeting-")
        || id.len() > 80
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(AppError::File("Некорректный ID конференции".into()));
    }
    Ok(root()?.join(id))
}
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
fn runtime_python() -> Result<PathBuf, AppError> {
    Ok(engine_root()?.join("runtime").join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }))
}
fn host_python() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("FERROFLUID_MEETINGS_PYTHON") {
        if PathBuf::from(&path).is_file() {
            return Some(path.into());
        }
    }
    for (name, prefix) in [
        ("py", vec!["-3.11"]),
        ("python3", vec![]),
        ("python", vec![]),
    ] {
        let output = command(name)
            .args(prefix)
            .args([
                "-c",
                "import sys; assert sys.version_info >= (3,10); print(sys.executable)",
            ])
            .output()
            .ok();
        if let Some(output) = output {
            if output.status.success() {
                return Some(String::from_utf8_lossy(&output.stdout).trim().into());
            }
        }
    }
    None
}
fn python() -> Result<PathBuf, AppError> {
    let runtime = runtime_python()?;
    if runtime.is_file() {
        return Ok(runtime);
    }
    host_python().ok_or_else(|| {
        AppError::Settings("Для конференций нужен Python 3.10+ (рекомендуется 3.11).".into())
    })
}
fn ffmpeg() -> Option<PathBuf> {
    let mut options: Vec<PathBuf> = std::env::var("FERROFLUID_FFMPEG_BIN")
        .ok()
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            options.push(parent.join("binaries").join("ffmpeg.exe"));
        }
    }
    options.push(PathBuf::from("src-tauri/binaries/ffmpeg.exe"));
    options.push(PathBuf::from(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }));
    options.into_iter().find(|path| {
        command(path)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}
fn read(id: &str) -> Result<Meeting, AppError> {
    serde_json::from_slice(&fs::read(directory(id)?.join("meeting.json"))?)
        .map_err(|e| AppError::Storage(e.to_string()))
}
fn store(meeting: &Meeting) -> Result<(), AppError> {
    let dir = directory(&meeting.id)?;
    fs::create_dir_all(&dir)?;
    let bytes = serde_json::to_vec_pretty(meeting).map_err(|e| AppError::Storage(e.to_string()))?;
    let temp = dir.join("meeting.tmp");
    fs::write(&temp, bytes)?;
    fs::rename(temp, dir.join("meeting.json"))?;
    Ok(())
}

// Only one meeting job can run at a time, independently of microphone dictation.
struct JobGuard(Arc<AtomicBool>);
impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn job(state: &MeetingsState) -> Result<JobGuard, AppError> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(AppError::Transcription(
            "Обработка конференции уже идёт.".into(),
        ));
    }
    state.cancel.store(false, Ordering::SeqCst);
    if let Ok(mut value) = state.progress.lock() {
        *value = None;
    }
    Ok(JobGuard(state.busy.clone()))
}
fn worker(
    app: &AppHandle,
    cancel: &AtomicBool,
    progress: &Mutex<Option<serde_json::Value>>,
    args: &[String],
    setup: bool,
) -> Result<(), AppError> {
    fs::create_dir_all(engine_root()?)?;
    let script = engine_root()?.join("meetings_worker.py");
    fs::write(&script, include_str!("meetings_worker.py"))?;
    fs::write(
        engine_root()?.join("meetings_advanced.py"),
        include_str!("meetings_advanced.py"),
    )?;
    let executable = if setup {
        host_python().ok_or_else(|| {
            AppError::Settings("Установите Python 3.11 для локального движка.".into())
        })?
    } else {
        python()?
    };
    let errors = tempfile::tempfile()?;
    let mut cmd = command(executable);
    cmd.arg("-u")
        .arg(&script)
        .args(args)
        .env("PYTHONUTF8", "1")
        .stdout(Stdio::piped())
        .stderr(errors.try_clone()?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
    let status = loop {
        for line in receiver.try_iter() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Ok(mut latest) = progress.lock() {
                    *latest = Some(value.clone());
                }
                let _ = app.emit("meeting-progress", value);
            }
        }
        if cancel.load(Ordering::SeqCst) {
            #[cfg(windows)]
            {
                let _ = command("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(unix)]
            {
                let _ = command("kill")
                    .args(["-TERM", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(AppError::Transcription(
                "Обработка отменена. Сохранённые данные доступны.".into(),
            ));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = reader.join();
    if !status.success() {
        use std::io::{Read, Seek};
        let mut errors = errors;
        errors.rewind()?;
        let mut text = String::new();
        errors.read_to_string(&mut text)?;
        return Err(AppError::Transcription(
            text.chars()
                .rev()
                .take(2500)
                .collect::<String>()
                .chars()
                .rev()
                .collect(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn meeting_tools_status(
    state: tauri::State<'_, MeetingsState>,
) -> Result<ToolsStatus, AppError> {
    let busy = state.busy.load(Ordering::SeqCst);
    let progress = state.progress.lock().ok().and_then(|p| p.clone());
    tauri::async_runtime::spawn_blocking(move || {
        Ok(ToolsStatus {
            python_ready: python().is_ok(),
            ffmpeg_ready: ffmpeg().is_some(),
            diarization_ready: engine_root()?.join("ready").is_file()
                && runtime_python()?.is_file()
                && engine_root()?.join("segmentation.onnx").is_file()
                && engine_root()?.join("embedding.onnx").is_file(),
            busy,
            progress,
        })
    })
    .await
    .map_err(|e| AppError::Settings(e.to_string()))?
}
#[tauri::command]
pub fn cancel_meeting_job(state: tauri::State<MeetingsState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn setup_meeting_tools(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
) -> Result<(), AppError> {
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        worker(
            &app,
            &cancel,
            &progress,
            &[
                "setup".into(),
                "--root".into(),
                engine_root()?.to_string_lossy().into(),
            ],
            true,
        )
    })
    .await
    .map_err(|e| AppError::Settings(e.to_string()))?
}
#[tauri::command]
pub async fn import_meeting(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
    source: String,
) -> Result<Meeting, AppError> {
    let source = PathBuf::from(source).canonicalize()?;
    let extension = source
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !source.is_file()
        || !["m4a", "wav", "mp3", "flac", "ogg", "aac", "mp4"].contains(&extension.as_str())
    {
        return Err(AppError::File(
            "Выберите аудиофайл M4A, WAV, MP3, FLAC, OGG, AAC или MP4.".into(),
        ));
    }
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let id = format!(
            "meeting-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let dir = directory(&id)?;
        fs::create_dir_all(&dir)?;
        let output = dir.join("import.json");
        let converter = ffmpeg().ok_or_else(|| {
            AppError::Settings(
                "Для импорта M4A нужен FFmpeg в PATH или FERROFLUID_FFMPEG_BIN.".into(),
            )
        })?;
        worker(
            &app,
            &cancel,
            &progress,
            &[
                "import".into(),
                "--source".into(),
                source.to_string_lossy().into(),
                "--ffmpeg".into(),
                converter.to_string_lossy().into(),
                "--output".into(),
                output.to_string_lossy().into(),
            ],
            false,
        )?;
        let info: serde_json::Value = serde_json::from_slice(&fs::read(&output)?)
            .map_err(|e| AppError::File(e.to_string()))?;
        let meeting = Meeting {
            id,
            title: source
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            source_name: source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            audio_path: dir.join("audio.wav").to_string_lossy().into(),
            duration_seconds: info["durationSeconds"].as_f64().unwrap_or_default(),
            created_at: chrono::Utc::now().to_rfc3339(),
            language: "auto".into(),
            model_name: None,
            speakers: vec![],
            turns: vec![],
            segments: vec![],
            calibration: None,
        };
        store(&meeting)?;
        Ok(meeting)
    })
    .await
    .map_err(|e| AppError::File(e.to_string()))?
}
#[tauri::command]
pub async fn analyze_meeting(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
    id: String,
    speaker_count: u32,
) -> Result<Meeting, AppError> {
    if speaker_count > 20 {
        return Err(AppError::Settings(
            "Допустимо от 1 до 20 участников или автоопределение.".into(),
        ));
    }
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || { let _guard = guard;
        let mut meeting = read(&id)?;
        if !meeting.segments.is_empty() { return Err(AppError::Settings("Повторный поиск голосов доступен до транскрипции. Для новой обработки импортируйте запись повторно.".into())); }
        let output = directory(&id)?.join("turns.json");
        worker(&app, &cancel, &progress, &["analyze".into(), "--root".into(), engine_root()?.to_string_lossy().into(), "--audio".into(), meeting.audio_path.clone(), "--output".into(), output.to_string_lossy().into(), "--speakers".into(), speaker_count.to_string()], false)?;
        meeting.turns = serde_json::from_slice(&fs::read(output)?).map_err(|e| AppError::File(e.to_string()))?;
        let mut ids = Vec::<String>::new();
        meeting.speakers = meeting.turns.iter().filter_map(|turn| {
            let speaker_id = turn.speaker_id.as_ref()?;
            if ids.contains(speaker_id) { return None; } ids.push(speaker_id.clone());
            let sample = meeting.turns.iter().filter(|t| t.speaker_id == turn.speaker_id).max_by(|a,b| (a.end-a.start).total_cmp(&(b.end-b.start))).unwrap();
            Some(Speaker { id: speaker_id.clone(), name: format!("Участник {}", ids.len()), sample_start: sample.start, sample_end: (sample.start + 8.0).min(sample.end) })
        }).collect();
        meeting.calibration = None;
        store(&meeting)?; Ok(meeting)
    }).await.map_err(|e| AppError::Transcription(e.to_string()))?
}
#[tauri::command]
pub async fn transcribe_meeting(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
    app_state: tauri::State<'_, AppState>,
    id: String,
    language: String,
) -> Result<Meeting, AppError> {
    if !["auto", "ru", "en", "uk", "zh", "es"].contains(&language.as_str()) {
        return Err(AppError::Settings("Неподдерживаемый язык".into()));
    }
    let settings = app_state
        .settings
        .lock()
        .map_err(|e| AppError::Settings(e.to_string()))?
        .clone();
    let model = settings
        .model_path
        .filter(|p| p.is_file())
        .ok_or(AppError::ModelNotFound)?;
    let engines = whisper_binary_candidates();
    if engines.is_empty() {
        return Err(AppError::WhisperBinaryNotFound);
    }
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut meeting = read(&id)?;
        if meeting.turns.is_empty() {
            return Err(AppError::Settings(
                "Сначала найдите голоса участников.".into(),
            ));
        }
        let dir = directory(&id)?;
        let turns = dir.join("turns-input.json");
        let output = dir.join("segments.json");
        fs::write(
            &turns,
            serde_json::to_vec(&meeting.turns).map_err(|e| AppError::Storage(e.to_string()))?,
        )?;
        let mut args = vec![
            "transcribe".into(),
            "--audio".into(),
            meeting.audio_path.clone(),
            "--output".into(),
            output.to_string_lossy().into(),
            "--turns".into(),
            turns.to_string_lossy().into(),
            "--model".into(),
            model.to_string_lossy().into(),
            "--language".into(),
            language.clone(),
            "--engines".into(),
        ];
        args.extend(
            engines
                .into_iter()
                .map(|e| e.path.to_string_lossy().into_owned()),
        );
        worker(&app, &cancel, &progress, &args, false)?;
        meeting.segments = serde_json::from_slice(&fs::read(output)?)
            .map_err(|e| AppError::File(e.to_string()))?;
        meeting.language = language;
        meeting.model_name = model.file_name().map(|f| f.to_string_lossy().into_owned());
        store(&meeting)?;
        Ok(meeting)
    })
    .await
    .map_err(|e| AppError::Transcription(e.to_string()))?
}
#[tauri::command]
pub fn list_meetings() -> Result<Vec<Meeting>, AppError> {
    fs::create_dir_all(root()?)?;
    let mut meetings = vec![];
    for entry in fs::read_dir(root()?)?.flatten() {
        if let Some(id) = entry
            .file_name()
            .to_str()
            .filter(|id| id.starts_with("meeting-"))
        {
            if let Ok(meeting) = read(id) {
                meetings.push(meeting);
            }
        }
    }
    meetings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(meetings)
}

#[tauri::command]
pub async fn calibrate_meeting(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
    id: String,
    references: Vec<VoiceReference>,
) -> Result<Meeting, AppError> {
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut meeting = read(&id)?;
        if references.is_empty() || references.len() > 80 || references.iter().any(|r| {
            r.name.trim().is_empty() || !r.start.is_finite() || !r.end.is_finite()
                || r.start < 0.0 || r.end - r.start < 7.0 || r.end - r.start > 120.0
                || r.end > meeting.duration_seconds
                || !meeting.speakers.iter().any(|s| s.id == r.speaker_id)
        }) {
            return Err(AppError::Settings("Для каждого голоса укажите имя и 7–120 секунд чистой речи в пределах записи.".into()));
        }
        for (i, r) in references.iter().enumerate() {
            for other in &references[..i] {
                if (r.speaker_id == other.speaker_id && r.name != other.name)
                    || (r.speaker_id != other.speaker_id && r.start < other.end && other.start < r.end) {
                    return Err(AppError::Settings("Образцы разных участников не должны пересекаться; одному голосу нужно одно имя.".into()));
                }
            }
        }
        if meeting.speakers.iter().any(|s| !references.iter().any(|r| r.speaker_id == s.id)) {
            return Err(AppError::Settings("Добавьте подтверждённый образец каждого участника или объедините лишние группы голосов.".into()));
        }
        let dir = directory(&id)?;
        let input = dir.join("calibration-input.json");
        let refs = dir.join("calibration-references.json");
        let output = dir.join("calibration-result.json");
        fs::write(&input, serde_json::to_vec(&meeting).map_err(|e| AppError::Storage(e.to_string()))?)?;
        fs::write(&refs, serde_json::to_vec(&references).map_err(|e| AppError::Storage(e.to_string()))?)?;
        worker(&app, &cancel, &progress, &[
            "calibrate".into(), "--root".into(), engine_root()?.to_string_lossy().into(),
            "--meeting".into(), input.to_string_lossy().into(), "--references".into(), refs.to_string_lossy().into(),
            "--output".into(), output.to_string_lossy().into(),
        ], false)?;
        let result: CalibrationResult = serde_json::from_slice(&fs::read(output)?).map_err(|e| AppError::Storage(e.to_string()))?;
        if result.segments.len() != meeting.segments.len() || result.segments.iter().zip(&meeting.segments).any(|(new, old)| new.id != old.id || new.text != old.text || new.start != old.start || new.end != old.end) {
            return Err(AppError::Storage("Калибровка не должна изменять текст и таймкоды транскрипта.".into()));
        }
        fs::copy(dir.join("meeting.json"), dir.join(format!("before-calibration-{}.json", chrono::Utc::now().timestamp_millis())))?;
        for speaker in &mut meeting.speakers {
            let sample = references.iter().find(|r| r.speaker_id == speaker.id).unwrap();
            speaker.name = sample.name.trim().into();
            speaker.sample_start = sample.start;
            speaker.sample_end = (sample.start + 8.0).min(sample.end);
        }
        meeting.turns = result.turns;
        meeting.segments = result.segments;
        meeting.calibration = Some(result.calibration);
        store(&meeting)?;
        Ok(meeting)
    }).await.map_err(|e| AppError::Transcription(e.to_string()))?
}
#[tauri::command]
pub async fn refine_meeting(
    app: AppHandle,
    state: tauri::State<'_, MeetingsState>,
    id: String,
    language: String,
) -> Result<Meeting, AppError> {
    if !["ru", "en", "uk", "zh", "es"].contains(&language.as_str()) {
        return Err(AppError::Settings(
            "Выберите язык для уточнения границ слов.".into(),
        ));
    }
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    let progress = state.progress.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let source = read(&id)?;
        if source.calibration.is_none() || source.segments.is_empty() {
            return Err(AppError::Settings(
                "Сначала подтвердите образцы участников и создайте транскрипт.".into(),
            ));
        }
        let dir = directory(&id)?;
        let input = dir.join("precise-input.json");
        let output = dir.join("precise-result.json");
        fs::write(
            &input,
            serde_json::to_vec(&source).map_err(|e| AppError::Storage(e.to_string()))?,
        )?;
        let mut args = vec![
            "precise".into(),
            "--root".into(),
            engine_root()?.to_string_lossy().into(),
            "--meeting".into(),
            input.to_string_lossy().into(),
            "--output".into(),
            output.to_string_lossy().into(),
            "--language".into(),
            language.clone(),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let native = parent.join("binaries/nemo/nemo-speech.exe");
                if native.is_file() {
                    args.extend(["--nemo".into(), native.to_string_lossy().into()]);
                }
            }
        }
        worker(&app, &cancel, &progress, &args, false)?;
        let result: CalibrationResult = serde_json::from_slice(&fs::read(output)?)
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let canonical = |segments: &[Segment]| {
            segments
                .iter()
                .map(|s| s.text.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join(" ")
        };
        if canonical(&source.segments) != canonical(&result.segments)
            || source
                .segments
                .iter()
                .filter(|s| s.speaker_locked)
                .any(|old| {
                    !result.segments.iter().any(|new| {
                        new.id == old.id
                            && new.text == old.text
                            && new.speaker_id == old.speaker_id
                            && new.speaker_locked
                            && new.start == old.start
                            && new.end == old.end
                    })
                })
            || result.segments.iter().any(|s| {
                !s.start.is_finite()
                    || !s.end.is_finite()
                    || s.start < 0.0
                    || s.end <= s.start
                    || s.end > source.duration_seconds + 1.0
                    || s.speaker_id
                        .as_ref()
                        .is_some_and(|id| !source.speakers.iter().any(|speaker| &speaker.id == id))
            })
        {
            return Err(AppError::Storage(
                "Уточнение не должно терять слова или менять закреплённые реплики.".into(),
            ));
        }
        let mut refined = source.clone();
        refined.id = format!("meeting-{}-refined", chrono::Utc::now().timestamp_micros());
        refined.title = format!("{} · уточнённая версия", source.title);
        refined.created_at = chrono::Utc::now().to_rfc3339();
        refined.language = language;
        refined.turns = result.turns;
        refined.segments = result.segments;
        refined.calibration = Some(result.calibration);
        store(&refined)?;
        Ok(refined)
    })
    .await
    .map_err(|e| AppError::Transcription(e.to_string()))?
}

#[tauri::command]
pub fn save_meeting(state: tauri::State<MeetingsState>, meeting: Meeting) -> Result<(), AppError> {
    let _guard = job(&state)?;
    let mut saved = read(&meeting.id)?;
    if meeting.title.trim().is_empty() || meeting.speakers.iter().any(|s| s.name.trim().is_empty())
    {
        return Err(AppError::Settings(
            "Название и имена участников не могут быть пустыми.".into(),
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if meeting.speakers.len() > 40
        || meeting.speakers.iter().any(|s| {
            !ids.insert(&s.id)
                || !s.sample_start.is_finite()
                || !s.sample_end.is_finite()
                || s.sample_start < 0.0
                || s.sample_end <= s.sample_start
                || s.sample_end > saved.duration_seconds + 1.0
                || (!saved.speakers.iter().any(|old| old.id == s.id)
                    && !s.id.starts_with("speaker-manual-"))
        })
        || meeting.turns.len() != saved.turns.len()
        || meeting.turns.iter().zip(&saved.turns).any(|(turn, old)| {
            turn.start != old.start
                || turn.end != old.end
                || turn
                    .speaker_id
                    .as_ref()
                    .is_some_and(|id| !meeting.speakers.iter().any(|s| &s.id == id))
        })
        || meeting.segments.iter().any(|s| {
            !s.start.is_finite()
                || !s.end.is_finite()
                || s.start < 0.0
                || s.end <= s.start
                || s.end > saved.duration_seconds + 1.0
                || s.speaker_id
                    .as_ref()
                    .is_some_and(|id| !meeting.speakers.iter().any(|speaker| &speaker.id == id))
        })
    {
        return Err(AppError::Storage(
            "Некорректные участники или таймкоды.".into(),
        ));
    }
    saved.title = meeting.title;
    saved.speakers = meeting.speakers;
    saved.turns = meeting.turns;
    saved.segments = meeting.segments;
    store(&saved)
}
#[tauri::command]
pub async fn export_meeting_file(text: String, extension: String) -> Result<bool, AppError> {
    if !["txt", "srt", "json"].contains(&extension.as_str()) {
        return Err(AppError::File("Неподдерживаемый формат".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Транскрипт конференции", &[&extension])
            .set_file_name(format!("meeting.{extension}"))
            .save_file()
        {
            fs::write(path, text)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .await
    .map_err(|e| AppError::File(e.to_string()))?
}
