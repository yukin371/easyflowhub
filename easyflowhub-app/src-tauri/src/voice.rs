//! Windows-first microphone capture.
//!
//! The input stream and WAV writer live on a dedicated native thread. Tauri
//! commands only send lifecycle messages and read the session snapshot, so
//! cpal's platform stream never crosses the global state boundary.

use chrono::Utc;
use serde::Serialize;
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;
#[cfg(target_os = "windows")]
use tauri::Manager;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub duration_ms: u64,
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub audio_path: Option<String>,
    pub inbox_item_id: Option<String>,
    pub error: Option<String>,
}

impl Default for VoiceSession {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "录音".to_string(),
            status: "idle".to_string(),
            started_at: None,
            stopped_at: None,
            duration_ms: 0,
            device_name: None,
            sample_rate: 48_000,
            channels: 1,
            audio_path: None,
            inbox_item_id: None,
            error: None,
        }
    }
}

#[cfg(target_os = "windows")]
enum VoiceCommand {
    Pause(mpsc::SyncSender<Result<(), String>>),
    Resume(mpsc::SyncSender<Result<(), String>>),
    Stop(mpsc::SyncSender<Result<(), String>>),
}

#[cfg(target_os = "windows")]
type CommandSender = mpsc::Sender<VoiceCommand>;

#[cfg(not(target_os = "windows"))]
type CommandSender = ();

struct VoiceRuntime {
    session: VoiceSession,
    command_tx: Option<CommandSender>,
}

fn runtime() -> &'static Mutex<VoiceRuntime> {
    static RUNTIME: OnceLock<Mutex<VoiceRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(VoiceRuntime { session: VoiceSession::default(), command_tx: None }))
}

#[cfg(not(target_os = "windows"))]
fn unsupported() -> String {
    "当前平台未启用 Windows 麦克风录音后端。".to_string()
}

fn current_session() -> Result<VoiceSession, String> {
    runtime().lock().map(|runtime| runtime.session.clone()).map_err(|_| "录音状态不可用。".to_string())
}

fn update_session<F>(id: &str, change: F)
where
    F: FnOnce(&mut VoiceSession),
{
    if let Ok(mut runtime) = runtime().lock() {
        if runtime.session.id == id {
            change(&mut runtime.session);
        }
    }
}

#[cfg(target_os = "windows")]
fn clear_command_sender(id: &str) {
    if let Ok(mut runtime) = runtime().lock() {
        if runtime.session.id == id {
            runtime.command_tx = None;
        }
    }
}

#[cfg(target_os = "windows")]
fn mark_failed(id: &str, error: String, audio_path: Option<String>, duration_ms: u64) {
    update_session(id, |session| {
        session.status = "failed".to_string();
        session.stopped_at = Some(Utc::now().to_rfc3339());
        session.duration_ms = duration_ms;
        session.audio_path = audio_path;
        session.error = Some(error);
    });
    clear_command_sender(id);
}

#[cfg(target_os = "windows")]
fn set_callback_error(target: &std::sync::Arc<Mutex<Option<String>>>, error: String) {
    if let Ok(mut value) = target.lock() {
        if value.is_none() {
            *value = Some(error);
        }
    }
}

#[cfg(target_os = "windows")]
type WavWriter = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

#[cfg(target_os = "windows")]
fn write_i16(data: &[i16], channels: usize, writer: &std::sync::Arc<Mutex<Option<WavWriter>>>, callback_error: &std::sync::Arc<Mutex<Option<String>>>) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for frame in data.chunks(channels.max(1)) {
                if let Err(error) = writer.write_sample(frame[0]) {
                    set_callback_error(callback_error, format!("写入录音失败: {error}"));
                    break;
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn write_u16(data: &[u16], channels: usize, writer: &std::sync::Arc<Mutex<Option<WavWriter>>>, callback_error: &std::sync::Arc<Mutex<Option<String>>>) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for frame in data.chunks(channels.max(1)) {
                let sample = (frame[0] as i32 - 32_768) as i16;
                if let Err(error) = writer.write_sample(sample) {
                    set_callback_error(callback_error, format!("写入录音失败: {error}"));
                    break;
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn write_f32(data: &[f32], channels: usize, writer: &std::sync::Arc<Mutex<Option<WavWriter>>>, callback_error: &std::sync::Arc<Mutex<Option<String>>>) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.as_mut() {
            for frame in data.chunks(channels.max(1)) {
                let sample = (frame[0].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                if let Err(error) = writer.write_sample(sample) {
                    set_callback_error(callback_error, format!("写入录音失败: {error}"));
                    break;
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn build_i16_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    writer: std::sync::Arc<Mutex<Option<WavWriter>>>,
    callback_error: std::sync::Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String> {
    use cpal::traits::DeviceTrait;
    let callback_writer = writer.clone();
    let callback_error_for_data = callback_error.clone();
    let callback_error_for_stream = callback_error.clone();
    device.build_input_stream(
        config,
        move |data: &[i16], _| write_i16(data, channels, &callback_writer, &callback_error_for_data),
        move |error| set_callback_error(&callback_error_for_stream, error.to_string()),
        None,
    ).map_err(|error| format!("打开麦克风流失败: {error}"))
}

#[cfg(target_os = "windows")]
fn build_u16_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    writer: std::sync::Arc<Mutex<Option<WavWriter>>>,
    callback_error: std::sync::Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String> {
    use cpal::traits::DeviceTrait;
    let callback_writer = writer.clone();
    let callback_error_for_data = callback_error.clone();
    let callback_error_for_stream = callback_error.clone();
    device.build_input_stream(
        config,
        move |data: &[u16], _| write_u16(data, channels, &callback_writer, &callback_error_for_data),
        move |error| set_callback_error(&callback_error_for_stream, error.to_string()),
        None,
    ).map_err(|error| format!("打开麦克风流失败: {error}"))
}

#[cfg(target_os = "windows")]
fn build_f32_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    writer: std::sync::Arc<Mutex<Option<WavWriter>>>,
    callback_error: std::sync::Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String> {
    use cpal::traits::DeviceTrait;
    let callback_writer = writer.clone();
    let callback_error_for_data = callback_error.clone();
    let callback_error_for_stream = callback_error.clone();
    device.build_input_stream(
        config,
        move |data: &[f32], _| write_f32(data, channels, &callback_writer, &callback_error_for_data),
        move |error| set_callback_error(&callback_error_for_stream, error.to_string()),
        None,
    ).map_err(|error| format!("打开麦克风流失败: {error}"))
}

#[cfg(target_os = "windows")]
fn run_recording(
    app: tauri::AppHandle,
    id: String,
    title: String,
    receiver: mpsc::Receiver<VoiceCommand>,
    ready: mpsc::SyncSender<Result<(String, u32, u16), String>>,
    directory: std::path::PathBuf,
) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleRate;
    use std::fs;
    use std::sync::Arc;
    use std::time::Instant;

    let fail_before_ready = |error: String, audio_path: Option<String>| {
        let _ = ready.send(Err(error.clone()));
        mark_failed(&id, error, audio_path, 0);
    };
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(device) => device,
        None => return fail_before_ready("没有找到可用的默认麦克风。".to_string(), None),
    };
    let device_name = device.name().unwrap_or_else(|_| "默认麦克风".to_string());
    let supported = match device.supported_input_configs() {
        Ok(configs) => configs.filter(|config| {
            config.channels() >= 1
                && config.min_sample_rate().0 <= 48_000
                && config.max_sample_rate().0 >= 48_000
                && matches!(config.sample_format(), cpal::SampleFormat::I16 | cpal::SampleFormat::U16 | cpal::SampleFormat::F32)
        }).next(),
        Err(error) => return fail_before_ready(format!("读取麦克风格式失败: {error}"), None),
    };
    let Some(supported) = supported else {
        return fail_before_ready("默认麦克风不支持 48 kHz 输入。".to_string(), None);
    };
    let channels = supported.channels();
    let supported = supported.with_sample_rate(SampleRate(48_000));
    let config: cpal::StreamConfig = supported.config();
    fs::create_dir_all(&directory).map_err(|error| format!("创建录音目录失败: {error}")).unwrap_or_else(|error| return fail_before_ready(error, None));
    let part_path = directory.join(format!(".{id}.wav.part"));
    let final_path = directory.join(format!("{id}.wav"));
    let writer = match hound::WavWriter::create(&part_path, hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int }) {
        Ok(writer) => Arc::new(Mutex::new(Some(writer))),
        Err(error) => return fail_before_ready(format!("创建录音文件失败: {error}"), Some(part_path.to_string_lossy().into_owned())),
    };
    let callback_error = Arc::new(Mutex::new(None::<String>));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::I16 => build_i16_stream(&device, &config, channels as usize, writer.clone(), callback_error.clone()),
        cpal::SampleFormat::U16 => build_u16_stream(&device, &config, channels as usize, writer.clone(), callback_error.clone()),
        cpal::SampleFormat::F32 => build_f32_stream(&device, &config, channels as usize, writer.clone(), callback_error.clone()),
        _ => Err("不支持的麦克风采样格式。".to_string()),
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => return fail_before_ready(error, Some(part_path.to_string_lossy().into_owned())),
    };
    if let Err(error) = stream.play() {
        return fail_before_ready(format!("启动麦克风失败: {error}"), Some(part_path.to_string_lossy().into_owned()));
    }
    // If the command caller timed out and dropped the response channel, stop
    // immediately instead of leaving an orphaned capture thread running.
    if ready.send(Ok((device_name, 48_000, 1))).is_err() {
        drop(stream);
        let _ = writer.lock().ok().and_then(|mut writer| writer.take());
        mark_failed(&id, "录音初始化已取消。".to_string(), Some(part_path.to_string_lossy().into_owned()), 0);
        return;
    }
    // Keep duration as active recording time. A paused session should not gain
    // wall-clock time while no samples are being written.
    let mut active_since = Instant::now();
    let mut accumulated_active_ms = 0_u64;
    let mut is_paused = false;
    let mut stop_reply = None;
    loop {
        if let Some(error) = callback_error.lock().ok().and_then(|error| error.clone()) {
            drop(stream);
            let _ = writer.lock().ok().and_then(|mut writer| writer.take());
            let error = format!("麦克风录音失败: {error}");
            let duration_ms = accumulated_active_ms
                + if is_paused { 0 } else { active_since.elapsed().as_millis() as u64 };
            mark_failed(&id, error.clone(), Some(part_path.to_string_lossy().into_owned()), duration_ms);
            // A stop command may have been queued just as the callback failed.
            // Reply to it so the command does not sit until its 30s timeout.
            while let Ok(command) = receiver.try_recv() {
                match command {
                    VoiceCommand::Pause(reply) | VoiceCommand::Resume(reply) | VoiceCommand::Stop(reply) => {
                        let _ = reply.send(Err(error.clone()));
                    }
                }
            }
            return;
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(VoiceCommand::Pause(reply)) => {
                let result = stream.pause().map_err(|error| format!("暂停录音失败: {error}"));
                if result.is_ok() && !is_paused {
                    accumulated_active_ms += active_since.elapsed().as_millis() as u64;
                    update_session(&id, |session| session.duration_ms = accumulated_active_ms);
                    is_paused = true;
                }
                let _ = reply.send(result);
            }
            Ok(VoiceCommand::Resume(reply)) => {
                let result = stream.play().map_err(|error| format!("继续录音失败: {error}"));
                if result.is_ok() && is_paused {
                    active_since = Instant::now();
                    is_paused = false;
                }
                let _ = reply.send(result);
            }
            Ok(VoiceCommand::Stop(reply)) => {
                stop_reply = Some(reply);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let duration_ms = accumulated_active_ms
                    + if is_paused { 0 } else { active_since.elapsed().as_millis() as u64 };
                update_session(&id, |session| session.duration_ms = duration_ms);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    let duration_ms = accumulated_active_ms
        + if is_paused { 0 } else { active_since.elapsed().as_millis() as u64 };
    let callback_failure = callback_error.lock().ok().and_then(|error| error.clone());
    let writer = writer.lock().ok().and_then(|mut writer| writer.take());
    let Some(writer) = writer else {
        let error = "录音写入器不可用。".to_string();
        mark_failed(&id, error.clone(), Some(part_path.to_string_lossy().into_owned()), duration_ms);
        if let Some(reply) = stop_reply { let _ = reply.send(Err(error)); }
        return;
    };
    if let Some(error) = callback_failure {
        let error = format!("麦克风录音失败: {error}");
        mark_failed(&id, error.clone(), Some(part_path.to_string_lossy().into_owned()), duration_ms);
        if let Some(reply) = stop_reply { let _ = reply.send(Err(error)); }
        return;
    }
    if let Err(error) = writer.finalize() {
        let error = format!("完成 WAV 文件失败: {error}");
        mark_failed(&id, error.clone(), Some(part_path.to_string_lossy().into_owned()), duration_ms);
        if let Some(reply) = stop_reply { let _ = reply.send(Err(error)); }
        return;
    }
    if let Err(error) = fs::rename(&part_path, &final_path) {
        let error = format!("保存录音失败: {error}");
        mark_failed(&id, error.clone(), Some(part_path.to_string_lossy().into_owned()), duration_ms);
        if let Some(reply) = stop_reply { let _ = reply.send(Err(error)); }
        return;
    }
    let inbox = crate::inbox::capture_inbox_item(app, crate::inbox::CaptureInboxItemInput {
        title,
        content: String::new(),
        source: Some("voice".to_string()),
        context: Some("manager".to_string()),
        image_data_url: None,
        file_paths: Vec::new(),
        audio_file_path: Some(final_path.to_string_lossy().into_owned()),
    });
    match inbox {
        Ok(response) => update_session(&id, |session| {
            session.status = "saved".to_string();
            session.stopped_at = Some(Utc::now().to_rfc3339());
            session.duration_ms = duration_ms;
            session.audio_path = Some(final_path.to_string_lossy().into_owned());
            session.inbox_item_id = Some(response.item.id);
            session.error = None;
        }),
        Err(error) => mark_failed(&id, format!("录音已保存但加入收集箱失败: {error}"), Some(final_path.to_string_lossy().into_owned()), duration_ms),
    }
    clear_command_sender(&id);
    if let Some(reply) = stop_reply {
        let result = current_session().and_then(|session| if session.status == "saved" { Ok(()) } else { Err(session.error.unwrap_or_else(|| "录音保存失败。".to_string())) });
        let _ = reply.send(result);
    }
}

#[cfg(not(target_os = "windows"))]
fn run_recording(_app: tauri::AppHandle, id: String, _title: String, _receiver: mpsc::Receiver<()>, _ready: mpsc::SyncSender<Result<(String, u32, u16), String>>, _directory: std::path::PathBuf) {
    update_session(&id, |session| { session.status = "failed".to_string(); session.error = Some(unsupported()); });
}

#[cfg(target_os = "windows")]
fn recording_directory(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let directory = app.path().app_data_dir().map_err(|error| format!("获取录音目录失败: {error}"))?.join("audio");
    std::fs::create_dir_all(&directory).map_err(|error| format!("创建录音目录失败: {error}"))?;
    Ok(directory)
}

#[tauri::command]
pub fn voice_get_state() -> Result<VoiceSession, String> { current_session() }

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn voice_start(app: tauri::AppHandle, title: Option<String>) -> Result<VoiceSession, String> {
    let directory = recording_directory(&app)?;
    let title = title.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "录音".to_string());
    let (sender, receiver) = mpsc::channel();
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let session = {
        let mut runtime = runtime().lock().map_err(|_| "录音状态不可用。".to_string())?;
        if matches!(runtime.session.status.as_str(), "recording" | "paused" | "stopping") {
            return Err("已有录音正在进行。".to_string());
        }
        let session = VoiceSession { id: Uuid::new_v4().to_string(), title: title.clone(), status: "recording".to_string(), started_at: Some(Utc::now().to_rfc3339()), ..VoiceSession::default() };
        runtime.session = session.clone();
        runtime.command_tx = Some(sender);
        session
    };
    let id = session.id.clone();
    thread::spawn(move || run_recording(app, id, title, receiver, ready_sender, directory));
    match ready_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok((device_name, sample_rate, channels))) => {
            update_session(&session.id, |current| { current.device_name = Some(device_name); current.sample_rate = sample_rate; current.channels = channels; });
            current_session()
        }
        Ok(Err(error)) => Err(error),
        Err(_) => {
            mark_failed(&session.id, "初始化麦克风超时。".to_string(), None, 0);
            Err("初始化麦克风超时。".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn voice_start(_app: tauri::AppHandle, _title: Option<String>) -> Result<VoiceSession, String> { Err(unsupported()) }

#[cfg(target_os = "windows")]
fn send_control(command: VoiceCommand, expected: &str, next: &str) -> Result<VoiceSession, String> {
    let (id, sender) = {
        let runtime = runtime().lock().map_err(|_| "录音状态不可用。".to_string())?;
        if runtime.session.status != expected { return Err("当前录音状态不允许此操作。".to_string()); }
        (runtime.session.id.clone(), runtime.command_tx.clone().ok_or_else(|| "录音会话不可用。".to_string())?)
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    match command {
        VoiceCommand::Pause(_) => sender.send(VoiceCommand::Pause(reply_sender)).map_err(|_| "录音线程已退出。".to_string())?,
        VoiceCommand::Resume(_) => sender.send(VoiceCommand::Resume(reply_sender)).map_err(|_| "录音线程已退出。".to_string())?,
        VoiceCommand::Stop(_) => sender.send(VoiceCommand::Stop(reply_sender)).map_err(|_| "录音线程已退出。".to_string())?,
    }
    reply_receiver.recv_timeout(Duration::from_secs(10)).map_err(|_| "录音操作超时。".to_string())??;
    update_session(&id, |session| session.status = next.to_string());
    current_session()
}

/// Stop is deliberately separate from the pause/resume helper. The recording
/// thread only replies after WAV finalization and Inbox capture have completed;
/// updating the state to `stopping` after that reply would overwrite `saved`.
#[cfg(target_os = "windows")]
fn stop_recording() -> Result<VoiceSession, String> {
    let (id, sender) = {
        let mut runtime = runtime().lock().map_err(|_| "录音状态不可用。".to_string())?;
        if !matches!(runtime.session.status.as_str(), "recording" | "paused") {
            return Err("当前录音状态不允许停止。".to_string());
        }
        let id = runtime.session.id.clone();
        let sender = runtime.command_tx.clone().ok_or_else(|| "录音会话不可用。".to_string())?;
        runtime.session.status = "stopping".to_string();
        (id, sender)
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    sender.send(VoiceCommand::Stop(reply_sender)).map_err(|_| {
        update_session(&id, |session| {
            session.status = "failed".to_string();
            session.error = Some("录音线程已退出。".to_string());
        });
        clear_command_sender(&id);
        "录音线程已退出。".to_string()
    })?;
    reply_receiver.recv_timeout(Duration::from_secs(30)).map_err(|_| "录音停止超时。".to_string())??;
    current_session()
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn voice_pause() -> Result<VoiceSession, String> { send_control(VoiceCommand::Pause(mpsc::sync_channel(1).0), "recording", "paused") }

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn voice_pause() -> Result<VoiceSession, String> { Err(unsupported()) }

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn voice_resume() -> Result<VoiceSession, String> { send_control(VoiceCommand::Resume(mpsc::sync_channel(1).0), "paused", "recording") }

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn voice_resume() -> Result<VoiceSession, String> { Err(unsupported()) }

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn voice_stop() -> Result<VoiceSession, String> { stop_recording() }

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn voice_stop() -> Result<VoiceSession, String> { Err(unsupported()) }

#[tauri::command]
pub fn voice_open(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let session = current_session()?;
    if session.id != id || session.status != "saved" { return Err("当前没有可打开的录音附件。".to_string()); }
    let path = session.audio_path.ok_or_else(|| "录音文件路径缺失。".to_string())?;
    app.opener().open_path(path, None::<String>).map_err(|error| format!("打开录音失败: {error}"))
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn voice_open_privacy_settings(app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("ms-settings:privacy-microphone", None::<String>)
        .map_err(|error| format!("打开麦克风隐私设置失败: {error}"))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn voice_open_privacy_settings() -> Result<(), String> { Err(unsupported()) }

#[cfg(test)]
mod tests {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Status { Idle, Recording, Paused, Stopping, Saved, Failed }
    struct FakeBackend { status: Status }
    impl FakeBackend {
        fn start(&mut self) { self.status = Status::Recording; }
        fn pause(&mut self) { if self.status == Status::Recording { self.status = Status::Paused; } }
        fn resume(&mut self) { if self.status == Status::Paused { self.status = Status::Recording; } }
        fn stop(&mut self, success: bool) { if matches!(self.status, Status::Recording | Status::Paused) { self.status = Status::Stopping; self.status = if success { Status::Saved } else { Status::Failed }; } }
    }
    #[test]
    fn fake_backend_covers_recording_lifecycle() { let mut backend = FakeBackend { status: Status::Idle }; backend.start(); backend.pause(); backend.resume(); backend.stop(true); assert_eq!(backend.status, Status::Saved); }
    #[test]
    fn fake_backend_failure_never_reports_saved() { let mut backend = FakeBackend { status: Status::Idle }; backend.start(); backend.stop(false); assert_eq!(backend.status, Status::Failed); }
    #[test]
    fn fake_backend_can_stop_while_paused() {
        let mut backend = FakeBackend { status: Status::Idle };
        backend.start();
        backend.pause();
        backend.stop(true);
        assert_eq!(backend.status, Status::Saved);
    }
    #[test]
    fn fake_backend_rejects_invalid_transitions() {
        let mut backend = FakeBackend { status: Status::Idle };
        backend.pause();
        assert_eq!(backend.status, Status::Idle);
        backend.resume();
        assert_eq!(backend.status, Status::Idle);
        backend.stop(true);
        assert_eq!(backend.status, Status::Idle);
    }
}
