use crate::errors::AppError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{SampleFormat, WavSpec, WavWriter};
use serde::Serialize;
use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

type SharedWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioCaptureInfo {
    pub path: String,
    pub duration_seconds: f64,
}

pub struct AudioRecorder {
    stop_flag: Arc<AtomicBool>,
    writer: SharedWriter,
    join_handle: Option<thread::JoinHandle<Result<(), AppError>>>,
    output_path: PathBuf,
    started_at: Instant,
}

impl AudioRecorder {
    pub fn start(output_path: PathBuf) -> Result<Self, AppError> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or(AppError::NoMicrophone)?;
        let config = device
            .default_input_config()
            .map_err(|error| AppError::Audio(error.to_string()))?;

        let spec = WavSpec {
            channels: config.channels(),
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let writer = WavWriter::create(&output_path, spec)
            .map_err(|error| AppError::Audio(error.to_string()))?;
        let writer: SharedWriter = Arc::new(Mutex::new(Some(writer)));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let thread_writer = Arc::clone(&writer);
        let thread_stop = Arc::clone(&stop_flag);
        let (ready_tx, ready_rx) = mpsc::channel();

        let join_handle = thread::spawn(move || {
            let err_fn = |err| eprintln!("VoiceGlass audio stream error: {err}");
            let stream_config = config.clone().into();

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| write_f32(data, &thread_writer),
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| write_i16(data, &thread_writer),
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| write_u16(data, &thread_writer),
                    err_fn,
                    None,
                ),
                format => {
                    let _ = ready_tx.send(Err(AppError::Audio(format!(
                        "Unsupported sample format: {format:?}"
                    ))));
                    return Ok(());
                }
            }
            .map_err(|error| AppError::Audio(error.to_string()))?;

            stream
                .play()
                .map_err(|error| AppError::Audio(error.to_string()))?;
            let _ = ready_tx.send(Ok(()));

            while !thread_stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(25));
            }

            drop(stream);
            Ok(())
        });

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                stop_flag,
                writer,
                join_handle: Some(join_handle),
                output_path,
                started_at: Instant::now(),
            }),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(AppError::Audio(error.to_string())),
        }
    }

    pub fn stop(mut self) -> Result<AudioCaptureInfo, AppError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .map_err(|_| AppError::Audio("Recorder thread panicked.".into()))??;
        }

        let mut locked = self
            .writer
            .lock()
            .map_err(|_| AppError::Audio("Recorder writer lock was poisoned.".into()))?;
        if let Some(writer) = locked.take() {
            writer
                .finalize()
                .map_err(|error| AppError::Audio(error.to_string()))?;
        }

        Ok(AudioCaptureInfo {
            path: self.output_path.to_string_lossy().to_string(),
            duration_seconds: self.started_at.elapsed().as_secs_f64(),
        })
    }
}

fn write_i16(data: &[i16], writer: &SharedWriter) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for sample in data {
                let _ = writer.write_sample(*sample);
            }
        }
    }
}

fn write_u16(data: &[u16], writer: &SharedWriter) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for sample in data {
                let centered = *sample as i32 - i16::MAX as i32 - 1;
                let _ =
                    writer.write_sample(centered.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
            }
        }
    }
}

fn write_f32(data: &[f32], writer: &SharedWriter) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for sample in data {
                let scaled = (*sample * i16::MAX as f32).round() as i32;
                let _ = writer.write_sample(scaled.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
            }
        }
    }
}
