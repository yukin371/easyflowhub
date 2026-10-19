//! Post-recording speech-to-text for Inbox audio attachments.
//!
//! Transcription is user initiated only: no audio is uploaded unless the user
//! explicitly confirms the provider and privacy notice in the UI. The audio
//! bytes never enter the WebView; this native owner reads the recorded file
//! and sends it to the local Relay, which owns provider credentials, routing
//! and failover (see `docs/decisions/ADR-0008-voice-transcription-and-relay.md`).
//!
//! The provider HTTP call is abstracted behind `TranscriptionTransport` so the
//! orchestration can be tested without a network, and a future local model or
//! CLI provider can be added without changing callers.

use reqwest::blocking::{Client, multipart::{Form, Part}};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::AppHandle;

/// Maximum audio size accepted for a single transcription request. A 48 kHz /
/// mono / 16-bit WAV is ~94 KiB per second, so this is roughly 4.5 minutes.
const MAX_AUDIO_BYTES: u64 = 25 * 1024 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 180;
const WAV_HEADER_BYTES: usize = 4096;
/// Same loopback default as the translation adapter; Relay owns credentials.
const DEFAULT_RELAY_URL: &str = "http://127.0.0.1:8787";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub model: String,
    pub attachment_id: String,
    pub audio_size_bytes: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxTranscriptView {
    pub attachment_id: String,
    pub text: String,
    pub model: Option<String>,
    pub created_at: String,
}

impl From<crate::inbox::InboxTranscript> for InboxTranscriptView {
    fn from(value: crate::inbox::InboxTranscript) -> Self {
        Self {
            attachment_id: value.attachment_id,
            text: value.text,
            model: value.model,
            created_at: value.created_at,
        }
    }
}

/// Request payload handed to a transport implementation.
pub struct TranscriptionRequest<'a> {
    pub audio_bytes: Vec<u8>,
    pub model: &'a str,
    pub language: Option<&'a str>,
}

/// Provider-side failure categories. Callers map these to fixed Chinese
/// messages; raw Relay or provider bodies are never surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionError {
    Connection,
    Timeout,
    Auth,
    RateLimited,
    EndpointUnavailable,
    ProviderUnavailable,
    RequestFailed,
    InvalidResponse,
    EmptyResult,
}

impl TranscriptionError {
    pub fn message(self) -> &'static str {
        match self {
            TranscriptionError::Connection => "无法连接 Relay，请确认本机 Relay 已启动且地址可访问。",
            TranscriptionError::Timeout => "转写超时（180 秒），请稍后重试或更换更快的模型。",
            TranscriptionError::Auth => "转写服务鉴权失败，请在 Relay 中检查 Provider 密钥和访问权限。",
            TranscriptionError::RateLimited => "转写请求过于频繁或额度不足，请稍后重试并检查 Provider 额度。",
            TranscriptionError::EndpointUnavailable => {
                "转写接口不可用：Relay 路由或上游 Provider 不支持音频转写，请检查模型与路由配置。"
            }
            TranscriptionError::ProviderUnavailable => "Relay 的上游转写服务暂时不可用，请检查 Provider 状态后重试。",
            TranscriptionError::RequestFailed => "转写请求未被接受，请检查 Relay 中的模型和路由配置。",
            TranscriptionError::InvalidResponse => "转写服务返回了无法识别的结果，请检查 Provider 是否兼容 OpenAI 音频接口。",
            TranscriptionError::EmptyResult => "转写结果为空，请确认录音包含语音或更换模型后重试。",
        }
    }
}

/// Provider transport abstraction. The first implementation posts an
/// OpenAI-compatible multipart request through the local Relay.
pub trait TranscriptionTransport {
    fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Result<String, TranscriptionError>;
}

pub struct RelayTransport {
    client: Client,
    base_url: String,
}

impl RelayTransport {
    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, base_url: DEFAULT_RELAY_URL.to_string() }
    }
}

/// Normalize a configured Relay base URL onto the transcription endpoint.
/// Accepts a bare host or a URL already ending in `/v1` or `/relay/v1`.
pub fn transcription_endpoint(base_url: &str) -> Result<String, TranscriptionError> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(TranscriptionError::Connection);
    }
    let url = reqwest::Url::parse(trimmed).map_err(|_| TranscriptionError::Connection)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(TranscriptionError::Connection);
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/v1".to_string()
    } else if path.ends_with("/v1") || path.ends_with("/relay/v1") {
        path.to_string()
    } else {
        format!("{path}/v1")
    };
    let mut endpoint = url;
    endpoint.set_path(&format!("{path}/audio/transcriptions"));
    Ok(endpoint.to_string())
}

/// Extract the transcript text from an OpenAI-compatible transcription
/// response. Only the known `text` field is trusted.
pub fn extract_transcript_text(payload: &str) -> Result<String, TranscriptionError> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| TranscriptionError::InvalidResponse)?;
    let text = value.get("text").and_then(|value| value.as_str()).unwrap_or("").trim();
    if text.is_empty() {
        return Err(TranscriptionError::EmptyResult);
    }
    Ok(text.to_string())
}

fn status_error(status: reqwest::StatusCode) -> TranscriptionError {
    if status == reqwest::StatusCode::NOT_FOUND {
        TranscriptionError::EndpointUnavailable
    } else if matches!(status.as_u16(), 401 | 403) {
        TranscriptionError::Auth
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        TranscriptionError::RateLimited
    } else if status.is_server_error() {
        TranscriptionError::ProviderUnavailable
    } else {
        TranscriptionError::RequestFailed
    }
}

impl TranscriptionTransport for RelayTransport {
    fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Result<String, TranscriptionError> {
        let endpoint = transcription_endpoint(&self.base_url)?;
        let mut form = Form::new()
            .text("model", request.model.to_string())
            .part(
                "file",
                Part::bytes(request.audio_bytes.clone())
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|_| TranscriptionError::InvalidResponse)?,
            );
        if let Some(language) = request.language.filter(|value| !value.trim().is_empty()) {
            form = form.text("language", language.trim().to_string());
        }

        let response = self
            .client
            .post(endpoint)
            .multipart(form)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    TranscriptionError::Timeout
                } else {
                    TranscriptionError::Connection
                }
            })?;

        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let payload = response.text().map_err(|_| TranscriptionError::InvalidResponse)?;
        extract_transcript_text(&payload)
    }
}

/// Best-effort duration estimate parsed from the RIFF/WAVE header. Only the
/// header prefix is read; the data chunk size and byte rate give seconds.
pub fn wav_duration_ms(header: &[u8]) -> Option<u64> {
    if header.len() < 12 || &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12_usize;
    let mut byte_rate: Option<u64> = None;
    let mut data_size: Option<u64> = None;
    while offset + 8 <= header.len() {
        let id = header.get(offset..offset + 4)?;
        let size = u32::from_le_bytes(header.get(offset + 4..offset + 8)?.try_into().ok()?) as u64;
        let body = offset + 8;
        if id == b"fmt " && body + 16 <= header.len() {
            byte_rate = Some(u32::from_le_bytes(header.get(body + 8..body + 12)?.try_into().ok()?) as u64);
        } else if id == b"data" {
            data_size = Some(size);
            break;
        }
        // Chunks are word aligned; skip the body plus padding.
        offset = body + size as usize + (size % 2) as usize;
    }
    let byte_rate = byte_rate?;
    let data_size = data_size?;
    if byte_rate == 0 {
        return None;
    }
    Some(data_size.saturating_mul(1000) / byte_rate)
}

/// Read the WAV header prefix for a duration estimate.
fn read_audio_duration(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut file = fs::File::open(path).ok()?;
    let mut header = vec![0_u8; WAV_HEADER_BYTES];
    let read = file.read(&mut header).unwrap_or(0);
    header.truncate(read);
    wav_duration_ms(&header)
}

/// Orchestrate a transcription run: resolve the audio attachment, apply size
/// guards, call the transport and persist the transcript. Shared by the
/// command and the tests.
pub(crate) fn run_transcription(
    conn: &mut rusqlite::Connection,
    assets_dir: &Path,
    item_id: &str,
    transport: &dyn TranscriptionTransport,
    model: &str,
    language: Option<&str>,
) -> Result<TranscriptionResult, String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("请先在「设置 → 转写」中选择转写模型。".to_string());
    }
    let (filename, _) = crate::inbox::find_audio_attachment_at(conn, item_id)
        .map_err(|error| error)?
        .ok_or_else(|| "该收集项没有可转写的音频附件。".to_string())?;
    let audio_path = resolve_attachment_path(assets_dir, &filename)?;
    let metadata = fs::metadata(&audio_path).map_err(|error| format!("无法读取录音文件: {error}"))?;
    if !metadata.is_file() {
        return Err("录音附件不是普通文件。".to_string());
    }
    if metadata.len() > MAX_AUDIO_BYTES {
        return Err("录音超过 25 MB 转写上限（约 4.5 分钟），请录短一些或更换支持长音频的 Provider。".to_string());
    }
    let audio_bytes = fs::read(&audio_path).map_err(|error| format!("读取录音文件失败: {error}"))?;
    let duration_ms = read_audio_duration(&audio_path);

    let text = transport
        .transcribe(&TranscriptionRequest {
            audio_bytes,
            model,
            language,
        })
        .map_err(|error| error.message().to_string())?;

    let attachment_id = crate::inbox::store_transcript_at(conn, assets_dir, item_id, &text, model)?;
    Ok(TranscriptionResult {
        text,
        model: model.to_string(),
        attachment_id,
        audio_size_bytes: metadata.len(),
        duration_ms,
    })
}

fn resolve_attachment_path(assets_dir: &Path, filename: &str) -> Result<PathBuf, String> {
    let safe_name = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == filename)
        .ok_or_else(|| "附件存储路径无效。".to_string())?;
    Ok(assets_dir.join(safe_name))
}

fn normalize_transcription_settings(settings: &crate::settings::TranscriptionSettings) -> (String, Option<String>) {
    let model = settings.model.trim().to_string();
    let language = {
        let value = settings.language.trim();
        if value.is_empty() { None } else { Some(value.to_string()) }
    };
    (model, language)
}

/// Transcribe the audio attachment of an Inbox item. The UI must have shown
/// the provider and privacy confirmation before calling this command.
#[tauri::command]
pub fn transcribe_audio(app: AppHandle, item_id: String) -> Result<TranscriptionResult, String> {
    let mut conn = crate::inbox::inbox_connection(&app)?;
    let assets_dir = crate::inbox::assets_dir(&app)?;
    let settings = crate::settings::get_settings(app)?.settings;
    let (model, language) = normalize_transcription_settings(&settings.transcription);
    let transport = RelayTransport::new();
    run_transcription(&mut conn, &assets_dir, &item_id, &transport, &model, language.as_deref())
}

/// Read the current transcript of an Inbox item, if the user transcribed it
/// before. Returns `None` when there is no transcript.
#[tauri::command]
pub fn get_transcript(app: AppHandle, item_id: String) -> Result<Option<InboxTranscriptView>, String> {
    let conn = crate::inbox::inbox_connection(&app)?;
    let assets_dir = crate::inbox::assets_dir(&app)?;
    Ok(crate::inbox::read_transcript_at(&conn, &assets_dir, &item_id)?.map(InboxTranscriptView::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use uuid::Uuid;

    struct FakeTransport {
        result: Result<String, TranscriptionError>,
        last_model: std::sync::Mutex<Option<String>>,
        last_language: std::sync::Mutex<Option<String>>,
        last_bytes: std::sync::Mutex<usize>,
    }

    impl FakeTransport {
        fn succeeding(text: &str) -> Self {
            Self {
                result: Ok(text.to_string()),
                last_model: std::sync::Mutex::new(None),
                last_language: std::sync::Mutex::new(None),
                last_bytes: std::sync::Mutex::new(0),
            }
        }

        fn failing(error: TranscriptionError) -> Self {
            Self {
                result: Err(error),
                last_model: std::sync::Mutex::new(None),
                last_language: std::sync::Mutex::new(None),
                last_bytes: std::sync::Mutex::new(0),
            }
        }
    }

    impl TranscriptionTransport for FakeTransport {
        fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Result<String, TranscriptionError> {
            *self.last_model.lock().unwrap() = Some(request.model.to_string());
            *self.last_language.lock().unwrap() = request.language.map(|value| value.to_string());
            *self.last_bytes.lock().unwrap() = request.audio_bytes.len();
            self.result.clone()
        }
    }

    fn memory_db_with_audio_item(assets_dir: &Path, audio_bytes: &[u8]) -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        crate::notes::init_db(&mut conn).expect("notes schema");
        crate::inbox::init_db(&mut conn).expect("inbox schema");
        conn.execute(
            "INSERT INTO inbox_items (id, kind, title, content, source, context, captured_at) VALUES ('item-1', 'audio', '录音', '', 'voice', 'manager', '2026-10-19T10:00:00Z')",
            [],
        )
        .expect("insert item");
        let filename = "audio-1.wav".to_string();
        conn.execute(
            "INSERT INTO inbox_attachments (id, item_id, kind, name, mime_type, filename, size_bytes, created_at) VALUES ('a1', 'item-1', 'audio', '录音.wav', 'audio/wav', ?1, ?2, '2026-10-19T10:00:00Z')",
            rusqlite::params![filename, audio_bytes.len() as i64],
        )
        .expect("insert audio attachment");
        fs::write(assets_dir.join(&filename), audio_bytes).expect("write audio file");
        conn
    }

    fn test_assets_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("easyflowhub-transcription-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("test assets directory");
        dir
    }

    /// Minimal 48 kHz / mono / 16-bit WAV header declaring 1 second of audio.
    fn one_second_wav_header() -> Vec<u8> {
        let byte_rate: u32 = 48_000 * 2;
        let data_size: u32 = byte_rate; // one second
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1_u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&48_000_u32.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes
    }

    #[test]
    fn wav_duration_ms_parses_standard_headers() {
        let header = one_second_wav_header();
        assert_eq!(wav_duration_ms(&header), Some(1000));
        assert_eq!(wav_duration_ms(b"not a wav file at all"), None);
        // A truncated header without a data chunk has no duration.
        assert_eq!(wav_duration_ms(&header[..20]), None);
    }

    #[test]
    fn transcript_text_requires_the_known_text_field() {
        assert_eq!(extract_transcript_text(r#"{"text":"你好"}"#).unwrap(), "你好");
        assert_eq!(extract_transcript_text(r#"{"text":"  "}"#).unwrap_err(), TranscriptionError::EmptyResult);
        assert_eq!(extract_transcript_text(r#"{"secret":"x"}"#).unwrap_err(), TranscriptionError::EmptyResult);
        assert_eq!(extract_transcript_text("not json").unwrap_err(), TranscriptionError::InvalidResponse);
    }

    #[test]
    fn endpoint_normalization_accepts_bare_host_and_v1_suffix() {
        assert_eq!(
            transcription_endpoint("http://127.0.0.1:8787").unwrap(),
            "http://127.0.0.1:8787/v1/audio/transcriptions"
        );
        assert_eq!(
            transcription_endpoint("http://127.0.0.1:8787/v1/").unwrap(),
            "http://127.0.0.1:8787/v1/audio/transcriptions"
        );
        assert!(transcription_endpoint("ftp://127.0.0.1").is_err());
        assert!(transcription_endpoint("   ").is_err());
    }

    #[test]
    fn status_errors_map_to_actionable_categories() {
        assert_eq!(status_error(reqwest::StatusCode::NOT_FOUND), TranscriptionError::EndpointUnavailable);
        assert_eq!(status_error(reqwest::StatusCode::UNAUTHORIZED), TranscriptionError::Auth);
        assert_eq!(status_error(reqwest::StatusCode::TOO_MANY_REQUESTS), TranscriptionError::RateLimited);
        assert_eq!(status_error(reqwest::StatusCode::BAD_GATEWAY), TranscriptionError::ProviderUnavailable);
        assert_eq!(status_error(reqwest::StatusCode::BAD_REQUEST), TranscriptionError::RequestFailed);
    }

    #[test]
    fn run_transcription_stores_text_and_passes_model_and_language() {
        let assets_dir = test_assets_dir();
        let mut conn = memory_db_with_audio_item(&assets_dir, &one_second_wav_header());
        let transport = FakeTransport::succeeding("转写文本");

        let result = run_transcription(&mut conn, &assets_dir, "item-1", &transport, "whisper-1", Some("zh"))
            .expect("transcription should succeed");

        assert_eq!(result.text, "转写文本");
        assert_eq!(result.model, "whisper-1");
        assert_eq!(result.duration_ms, Some(1000));
        assert_eq!(*transport.last_model.lock().unwrap(), Some("whisper-1".to_string()));
        assert_eq!(*transport.last_language.lock().unwrap(), Some("zh".to_string()));
        assert_eq!(*transport.last_bytes.lock().unwrap(), one_second_wav_header().len());

        let stored = crate::inbox::read_transcript_at(&conn, &assets_dir, "item-1")
            .expect("transcript should read back")
            .expect("transcript should exist");
        assert_eq!(stored.text, "转写文本");
        // The audio attachment itself is untouched.
        let audio_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_attachments WHERE kind = 'audio'", [], |row| row.get(0))
            .expect("audio count");
        assert_eq!(audio_count, 1);
        let _ = fs::remove_dir_all(&assets_dir);
    }

    #[test]
    fn run_transcription_rejects_missing_model_and_missing_audio() {
        let assets_dir = test_assets_dir();
        let mut conn = memory_db_with_audio_item(&assets_dir, &one_second_wav_header());
        let transport = FakeTransport::succeeding("转写文本");

        let missing_model = run_transcription(&mut conn, &assets_dir, "item-1", &transport, "  ", None)
            .expect_err("missing model should fail");
        assert!(missing_model.contains("设置"));

        conn.execute("DELETE FROM inbox_attachments WHERE kind = 'audio'", [])
            .expect("remove audio");
        let missing_audio = run_transcription(&mut conn, &assets_dir, "item-1", &transport, "whisper-1", None)
            .expect_err("missing audio should fail");
        assert!(missing_audio.contains("音频"));
        let _ = fs::remove_dir_all(&assets_dir);
    }

    #[test]
    fn run_transcription_failure_keeps_audio_and_reports_provider_message() {
        let assets_dir = test_assets_dir();
        let mut conn = memory_db_with_audio_item(&assets_dir, &one_second_wav_header());
        let transport = FakeTransport::failing(TranscriptionError::Connection);

        let error = run_transcription(&mut conn, &assets_dir, "item-1", &transport, "whisper-1", None)
            .expect_err("transport failure should surface");

        assert!(error.contains("Relay"));
        // No transcript is written and the audio attachment stays readable.
        assert!(crate::inbox::read_transcript_at(&conn, &assets_dir, "item-1")
            .expect("read should succeed")
            .is_none());
        let audio_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_attachments WHERE kind = 'audio'", [], |row| row.get(0))
            .expect("audio count");
        assert_eq!(audio_count, 1);
        let _ = fs::remove_dir_all(&assets_dir);
    }

    #[test]
    fn run_transcription_rejects_oversized_audio_before_sending() {
        let assets_dir = test_assets_dir();
        // The guard is size based; the payload itself does not need to be a
        // valid WAV because the size check happens first.
        let big = vec![0_u8; (MAX_AUDIO_BYTES + 1) as usize];
        let mut conn = memory_db_with_audio_item(&assets_dir, &big);
        let transport = FakeTransport::succeeding("should not run");

        let error = run_transcription(&mut conn, &assets_dir, "item-1", &transport, "whisper-1", None)
            .expect_err("oversized audio should fail");

        assert!(error.contains("25 MB"));
        assert_eq!(*transport.last_bytes.lock().unwrap(), 0);
        let _ = fs::remove_dir_all(&assets_dir);
    }
}
