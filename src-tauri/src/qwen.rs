use crate::{errors::AppError, stt::model_manager::tts_dir};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Seek, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct QwenState {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Value>>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfile {
    id: String,
    name: String,
    reference_text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QwenStatus {
    ready: bool,
    busy: bool,
    progress: Option<Value>,
    voices: Vec<VoiceProfile>,
    gpu: Option<String>,
}

fn root() -> Result<PathBuf, AppError> {
    Ok(tts_dir()?.join("qwen"))
}
fn python() -> Result<PathBuf, AppError> {
    Ok(root()?.join("runtime").join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    }))
}
fn profile_dir(id: &str) -> Result<PathBuf, AppError> {
    if !id.starts_with("voice-")
        || id.len() > 80
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(AppError::TextToSpeech("Некорректный ID голоса".into()));
    }
    Ok(root()?.join("voices").join(id))
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
fn emit(app: &AppHandle, state: &QwenState, value: Value) {
    if let Ok(mut progress) = state.progress.lock() {
        *progress = Some(value.clone());
    }
    let _ = app.emit("qwen-progress", value);
}
struct Guard(Arc<AtomicBool>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn job(state: &QwenState) -> Result<Guard, AppError> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(AppError::TextToSpeech("Qwen уже выполняет задачу.".into()));
    }
    state.cancel.store(false, Ordering::SeqCst);
    if let Ok(mut progress) = state.progress.lock() {
        *progress = None;
    }
    Ok(Guard(state.busy.clone()))
}

// Closing the app or crashing also closes this handle, terminating the whole
// worker tree, including uv's installer children. No orphan CUDA processes.
#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessJob {
    fn attach(child: &std::process::Child) -> Result<Self, AppError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
            {
                let error = std::io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error.into());
            }
            Ok(Self(handle))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn resume_worker(pid: u32) -> Result<(), AppError> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut more = Thread32First(snapshot, &mut entry);
        while more != 0 {
            if entry.th32OwnerProcessID == pid {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                CloseHandle(snapshot);
                if thread.is_null() {
                    return Err(std::io::Error::last_os_error().into());
                }
                let result = ResumeThread(thread);
                let error = std::io::Error::last_os_error();
                CloseHandle(thread);
                return if result == u32::MAX {
                    Err(error.into())
                } else {
                    Ok(())
                };
            }
            more = Thread32Next(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        Err(AppError::TextToSpeech(
            "Не удалось запустить поток движка Qwen.".into(),
        ))
    }
}

fn run(
    app: &AppHandle,
    state: &QwenState,
    mut cmd: Command,
    input: Option<Value>,
    timeout: Duration,
) -> Result<(), AppError> {
    let errors = tempfile::tempfile()?;
    cmd.env("PYTHONUTF8", "1")
        .env("PYTHONUNBUFFERED", "1")
        .env("UV_HTTP_TIMEOUT", "300")
        .env("HF_HUB_ETAG_TIMEOUT", "60")
        .env("HF_HUB_DOWNLOAD_TIMEOUT", "300")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(errors.try_clone()?);
    // Start suspended so the Python launcher cannot spawn an unowned child
    // between CreateProcess and AssignProcessToJobObject.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000004);
    }
    let mut child = cmd.spawn()?;
    #[cfg(windows)]
    let process_job = match ProcessJob::attach(&child) {
        Ok(value) => value,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    #[cfg(windows)]
    if let Err(error) = resume_worker(child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    if let Some(value) = input {
        let result = child
            .stdin
            .take()
            .unwrap()
            .write_all(value.to_string().as_bytes());
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    }
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
    let deadline = Instant::now() + timeout;
    let mut cancelled = false;
    let status = loop {
        for line in receiver.try_iter() {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                emit(app, state, value);
            }
        }
        if state.cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
            cancelled = true;
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(process_job.0, 1);
            }
            let _ = child.kill();
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = reader.join();
    for line in receiver.try_iter() {
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            emit(app, state, value);
        }
    }
    if cancelled {
        emit(
            app,
            state,
            json!({"message":"Задача отменена или превысила время ожидания. Можно повторить запуск.","error":true,"percentage":0}),
        );
        return Err(AppError::TextToSpeech(
            "Задача отменена или превысила время ожидания. Можно повторить запуск.".into(),
        ));
    }
    if !status.success() {
        let message = state
            .progress
            .lock()
            .ok()
            .and_then(|p| p.clone())
            .filter(|v| v["error"] == true)
            .and_then(|v| v["message"].as_str().map(str::to_owned));
        let mut errors = errors;
        errors.rewind()?;
        let mut details = String::new();
        errors.read_to_string(&mut details)?;
        return Err(AppError::TextToSpeech(message.unwrap_or_else(|| {
            details
                .chars()
                .rev()
                .take(1800)
                .collect::<String>()
                .chars()
                .rev()
                .collect()
        })));
    }
    Ok(())
}

fn ready() -> Result<bool, AppError> {
    let root = root()?;
    Ok(root.join("ready.json").is_file()
        && python()?.is_file()
        && ["base", "custom"].iter().all(|kind| {
            [
                "model.safetensors",
                "config.json",
                "vocab.json",
                "merges.txt",
                "tokenizer_config.json",
                "generation_config.json",
                "preprocessor_config.json",
                "speech_tokenizer/model.safetensors",
                "speech_tokenizer/config.json",
            ]
            .iter()
            .all(|file| root.join("models").join(kind).join(file).is_file())
        }))
}
fn profiles() -> Result<Vec<VoiceProfile>, AppError> {
    let folder = root()?.join("voices");
    if !folder.is_dir() {
        return Ok(vec![]);
    }
    let mut voices = Vec::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.join("reference.wav").is_file() {
            if let Ok(data) = fs::read(path.join("voice.json")) {
                if let Ok(profile) = serde_json::from_slice::<VoiceProfile>(&data) {
                    voices.push(profile);
                }
            }
        }
    }
    voices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(voices)
}
fn script() -> Result<PathBuf, AppError> {
    fs::create_dir_all(root()?)?;
    let script = root()?.join("qwen_worker.py");
    fs::write(&script, include_str!("qwen_worker.py"))?;
    Ok(script)
}
fn uv() -> Result<PathBuf, AppError> {
    let local = root()?.join("uv.exe");
    if local.is_file() {
        return Ok(local);
    }
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".local/bin/uv.exe");
        if path.is_file() {
            return Ok(path);
        }
    }
    if command("uv")
        .arg("--version")
        .output()
        .is_ok_and(|r| r.status.success())
    {
        return Ok("uv".into());
    }
    #[cfg(windows)]
    {
        let mut response = reqwest::blocking::Client::builder().timeout(Duration::from_secs(180)).build()
            .map_err(|e| AppError::Download(e.to_string()))?
            .get("https://github.com/astral-sh/uv/releases/download/0.8.22/uv-x86_64-pc-windows-msvc.zip")
            .send().and_then(|r| r.error_for_status()).map_err(|e| AppError::Download(e.to_string()))?;
        let mut file = tempfile::tempfile()?;
        std::io::copy(&mut response, &mut file)?;
        file.rewind()?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| AppError::Download(e.to_string()))?;
        let mut member = zip
            .by_name("uv.exe")
            .map_err(|e| AppError::Download(e.to_string()))?;
        let temp = local.with_extension("download");
        std::io::copy(&mut member, &mut fs::File::create(&temp)?)?;
        fs::rename(temp, &local)?;
        Ok(local)
    }
    #[cfg(not(windows))]
    {
        Err(AppError::Settings(
            "Установите uv для подготовки Qwen.".into(),
        ))
    }
}

#[tauri::command]
pub fn qwen_status(state: tauri::State<QwenState>) -> Result<QwenStatus, AppError> {
    let metadata: Value = fs::read(root()?.join("ready.json"))
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or(Value::Null);
    Ok(QwenStatus {
        ready: ready()?,
        busy: state.busy.load(Ordering::SeqCst),
        progress: state.progress.lock().ok().and_then(|p| p.clone()),
        voices: profiles()?,
        gpu: metadata["gpu"].as_str().map(str::to_owned),
    })
}
#[tauri::command]
pub fn cancel_qwen(state: tauri::State<QwenState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn setup_qwen(
    app: AppHandle,
    state: tauri::State<'_, QwenState>,
) -> Result<(), AppError> {
    let guard = job(&state)?;
    let state = QwenState {
        busy: state.busy.clone(),
        cancel: state.cancel.clone(),
        progress: state.progress.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard; let script = script()?;
        emit(&app, &state, json!({"message":"Подготавливаем отдельное окружение Qwen…", "percentage":2}));
        let uv = uv()?;
        if !python()?.is_file() {
            let mut cmd = command(&uv); cmd.args(["venv", "--python", "3.12"]).arg(root()?.join("runtime"));
            run(&app, &state, cmd, None, Duration::from_secs(900))?;
        }
        emit(&app, &state, json!({"message":"Устанавливаем PyTorch с поддержкой RTX 5080 · около 3,5 ГБ…", "percentage":10}));
        let mut cmd = command(&uv); cmd.args(["pip", "install", "--no-deps", "--python"]).arg(python()?).args(["torch==2.8.0", "torchaudio==2.8.0", "--index-url", "https://download.pytorch.org/whl/cu129"]);
        run(&app, &state, cmd, None, Duration::from_secs(3600))?;
        emit(&app, &state, json!({"message":"Устанавливаем движок озвучивания…", "percentage":25}));
        let mut cmd = command(&uv); cmd.args(["pip", "install", "--python"]).arg(python()?).args(["qwen-tts==0.1.1", "numpy==2.2.6", "torch==2.8.0", "torchaudio==2.8.0"]);
        run(&app, &state, cmd, None, Duration::from_secs(1800))?;
        let mut cmd = command(python()?); cmd.arg("-u").arg(script).arg("setup").arg("--root").arg(root()?);
        run(&app, &state, cmd, None, Duration::from_secs(7200))
    }).await.map_err(|e| AppError::TextToSpeech(e.to_string()))?
}

#[tauri::command]
pub async fn synthesize_qwen(
    app: AppHandle,
    state: tauri::State<'_, QwenState>,
    text: String,
    voice_id: String,
    style: String,
) -> Result<crate::stt::tts::TtsSynthesisResult, AppError> {
    if text.trim().is_empty() || text.chars().count() > 20000 {
        return Err(AppError::TextToSpeech(
            "Введите текст длиной от 1 до 20 000 символов.".into(),
        ));
    }
    if !ready()? {
        return Err(AppError::TextToSpeech(
            "Сначала скачайте Qwen во вкладке озвучивания.".into(),
        ));
    }
    let guard = job(&state)?;
    let state = QwenState {
        busy: state.busy.clone(),
        cancel: state.cancel.clone(),
        progress: state.progress.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let output = tts_dir()?.join("output").join(format!("qwen-{}.wav", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()));
        let mut input = json!({"text":text,"output":output,"style":style});
        let name = if voice_id.starts_with("voice-") {
            let dir = profile_dir(&voice_id)?;
            let profile: VoiceProfile = serde_json::from_slice(&fs::read(dir.join("voice.json"))?).map_err(|e| AppError::TextToSpeech(e.to_string()))?;
            input["reference"] = json!(dir.join("reference.wav")); input["referenceText"] = json!(profile.reference_text); profile.name
        } else {
            if !["Ryan","Aiden","Vivian","Serena","Uncle_Fu","Dylan","Eric","Ono_Anna","Sohee"].contains(&voice_id.as_str()) { return Err(AppError::TextToSpeech("Неизвестный голос Qwen.".into())); }
            input["speaker"] = json!(voice_id); voice_id
        };
        let mut cmd = command(python()?); cmd.arg("-u").arg(script()?).arg("synthesize").arg("--root").arg(root()?);
        let result = run(&app, &state, cmd, Some(input), Duration::from_secs(3600));
        if result.is_err() { let _ = fs::remove_file(output.with_extension("partial.wav")); }
        result?;
        if !output.is_file() { return Err(AppError::TextToSpeech("Qwen не создал аудиофайл.".into())); }
        emit(&app, &state, json!({"message":"Озвучивание готово", "percentage":100,"audioPath":output,"voiceName":name}));
        Ok(crate::stt::tts::TtsSynthesisResult { audio_path: output.to_string_lossy().into(), voice_name: name })
    }).await.map_err(|e| AppError::TextToSpeech(e.to_string()))?
}

#[tauri::command]
pub async fn import_qwen_voice(
    app: AppHandle,
    state: tauri::State<'_, QwenState>,
    source: String,
    name: String,
    reference_text: String,
) -> Result<VoiceProfile, AppError> {
    if name.trim().is_empty()
        || name.chars().count() > 80
        || reference_text.trim().is_empty()
        || reference_text.chars().count() > 2000
    {
        return Err(AppError::TextToSpeech(
            "Укажите имя (до 80 символов) и точный текст образца (до 2000 символов).".into(),
        ));
    }
    let guard = job(&state)?;
    let state = QwenState {
        busy: state.busy.clone(),
        cancel: state.cancel.clone(),
        progress: state.progress.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let id = format!("voice-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()); let dir = profile_dir(&id)?; fs::create_dir_all(&dir)?;
        let result = (|| {
            let ffmpeg = crate::meetings::ffmpeg().ok_or_else(|| AppError::TextToSpeech("Не найден FFmpeg для подготовки образца голоса.".into()))?;
            emit(&app, &state, json!({"message":"Подготавливаем образец голоса…","percentage":10}));
            let mut cmd = command(ffmpeg); cmd.args(["-nostdin","-v","error","-y","-i"]).arg(source).args(["-t","31","-vn","-ac","1","-ar","24000","-c:a","pcm_s16le"]).arg(dir.join("reference.wav"));
            run(&app, &state, cmd, None, Duration::from_secs(90))?;
            let wav = hound::WavReader::open(dir.join("reference.wav")).map_err(|e| AppError::TextToSpeech(e.to_string()))?;
            let duration = wav.duration() as f64 / wav.spec().sample_rate as f64;
            if !(3.0..=30.5).contains(&duration) { return Err(AppError::TextToSpeech("Выберите отдельный образец длиной от 3 до 30 секунд. Рекомендуется 10–30 секунд чистой речи одного человека.".into())); }
            let profile = VoiceProfile { id, name: name.trim().into(), reference_text: reference_text.trim().into() };
            fs::write(dir.join("voice.json"), serde_json::to_vec_pretty(&profile).unwrap())?;
            emit(&app, &state, json!({"message":"Голос сохранён", "percentage":100})); Ok(profile)
        })();
        if result.is_err() { let _ = fs::remove_dir_all(&dir); }
        result
    }).await.map_err(|e| AppError::TextToSpeech(e.to_string()))?
}

#[tauri::command]
pub fn delete_qwen_voice(state: tauri::State<QwenState>, voice_id: String) -> Result<(), AppError> {
    let _guard = job(&state)?;
    fs::remove_dir_all(profile_dir(&voice_id)?)?;
    Ok(())
}
#[tauri::command]
pub fn export_qwen_audio(source: String, destination: String) -> Result<(), AppError> {
    let source = fs::canonicalize(source)?;
    let allowed = fs::canonicalize(tts_dir()?.join("output"))?;
    if source.parent() != Some(allowed.as_path())
        || source.extension().is_none_or(|ext| ext != "wav")
    {
        return Err(AppError::File(
            "Можно экспортировать только созданный WAV.".into(),
        ));
    }
    if source != PathBuf::from(&destination) {
        fs::copy(source, destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_job_excludes_concurrent_work_and_recovers_after_failure() {
        let state = QwenState::default();
        let first = job(&state).unwrap();
        assert!(job(&state).is_err());
        state.cancel.store(true, Ordering::SeqCst);
        drop(first);
        let second = job(&state).unwrap();
        assert!(!state.cancel.load(Ordering::SeqCst));
        drop(second);
        assert!(!state.busy.load(Ordering::SeqCst));
    }

    #[test]
    fn qwen_profile_paths_reject_traversal_and_absolute_paths() {
        for id in [
            "../voice-1",
            "voice-../1",
            "voice-C:/other",
            "C:/voice-1",
            "voice-1/../../",
        ] {
            assert!(profile_dir(id).is_err());
        }
        assert!(profile_dir("voice-123").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn qwen_closing_job_terminates_worker() {
        use std::os::windows::process::CommandExt;
        let mut cmd = command("powershell.exe");
        cmd.creation_flags(0x08000004);
        let mut child = cmd
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let guard = ProcessJob::attach(&child).unwrap();
        resume_worker(child.id()).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(guard);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                panic!("Worker outlived its job handle");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
