use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

fn serialize_bytes_as_base64<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if bytes.is_empty() {
        serializer.serialize_none()
    } else {
        serializer.serialize_some(&STANDARD.encode(bytes))
    }
}

fn deserialize_bytes_from_base64<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => STANDARD
            .decode(&s)
            .map(Bytes::from)
            .map_err(serde::de::Error::custom),
        None => Ok(Bytes::new()),
    }
}

/// Response state machine
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ResponseState {
    #[default]
    Idle,
    Loading,
    Success,
    Cancelled,
    Error(String),
}

/// Content category for response body rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentCategory {
    Json,
    Html,
    Xml,
    Image,
    Audio,
    #[default]
    Text,
    Binary,
}
#[allow(dead_code)]
impl ContentCategory {
    /// Get the language name for syntax highlighting
    pub fn language(&self) -> &'static str {
        match self {
            ContentCategory::Json => "json",
            ContentCategory::Html => "html",
            ContentCategory::Xml => "xml",
            ContentCategory::Image => "text",
            ContentCategory::Audio => "text",
            ContentCategory::Text => "text",
            ContentCategory::Binary => "text",
        }
    }

    /// Get display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            ContentCategory::Json => "JSON",
            ContentCategory::Html => "HTML",
            ContentCategory::Xml => "XML",
            ContentCategory::Image => "Image",
            ContentCategory::Audio => "Audio",
            ContentCategory::Text => "Text",
            ContentCategory::Binary => "Binary",
        }
    }
}

/// Events emitted by ResponseEntity
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ResponseEvent {
    Loading,
    Received,
    Error(String),
    Cleared,
}

/// Shared response body storage. Cloning this value never copies the payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePayload {
    /// Text body (for text-based responses)
    #[serde(default)]
    body: Arc<str>,
    /// Raw bytes body (for binary responses like images)
    #[serde(
        serialize_with = "serialize_bytes_as_base64",
        deserialize_with = "deserialize_bytes_from_base64",
        default
    )]
    body_bytes: Bytes,
    /// Cached raw text, decoded from bytes only when needed.
    #[serde(skip)]
    cached_raw_body: Option<Arc<str>>,
    /// Cached pretty-printed representation, independent from the raw cache.
    #[serde(skip)]
    cached_formatted_body: Option<Arc<str>>,
    /// Hash of body content for efficient change detection.
    #[serde(skip)]
    body_hash: u64,
}

/// HTTP Response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    #[serde(flatten)]
    pub payload: ResponsePayload,
    pub body_size_bytes: usize,
    pub duration_ms: u64,
    pub content_type: Option<String>,
}

impl Default for ResponsePayload {
    fn default() -> Self {
        Self {
            body: Arc::from(""),
            body_bytes: Bytes::new(),
            cached_raw_body: None,
            cached_formatted_body: None,
            body_hash: 0,
        }
    }
}

impl Default for ResponseData {
    fn default() -> Self {
        Self {
            status_code: 0,
            status_text: String::new(),
            headers: HashMap::new(),
            payload: ResponsePayload::default(),
            body_size_bytes: 0,
            duration_ms: 0,
            content_type: None,
        }
    }
}

#[allow(dead_code)]
impl ResponseData {
    fn compute_stored_hash(body: &str, body_bytes: &[u8]) -> u64 {
        if body_bytes.is_empty() {
            Self::compute_hash(body.as_bytes())
        } else {
            Self::compute_hash(body_bytes)
        }
    }

    fn looks_like_audio(bytes: &[u8]) -> bool {
        // MP3 with ID3 tag
        if bytes.starts_with(b"ID3") {
            return true;
        }

        // MP3/AAC ADTS frame sync
        if bytes.len() > 1 && bytes[0] == 0xFF && (bytes[1] & 0xF0) == 0xF0 {
            return true;
        }

        // WAV
        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
            return true;
        }

        // OGG container (often Vorbis/Opus audio)
        if bytes.starts_with(b"OggS") {
            return true;
        }

        // FLAC
        if bytes.starts_with(b"fLaC") {
            return true;
        }

        // MP4/M4A family
        if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
            let brand = &bytes[8..12];
            if brand == b"M4A " || brand == b"M4B " || brand == b"isom" || brand == b"mp42" {
                return true;
            }
        }

        false
    }

    fn looks_like_image(bytes: &[u8]) -> bool {
        bytes.starts_with(b"\x89PNG\r\n\x1A\n")
            || bytes.starts_with(b"\xFF\xD8\xFF")
            || bytes.starts_with(b"GIF87a")
            || bytes.starts_with(b"GIF89a")
            || bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP"
            || bytes.starts_with(b"BM")
    }

    fn looks_like_text(bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return true;
        }

        let sample_len = bytes.len().min(512);
        let sample = &bytes[..sample_len];

        if std::str::from_utf8(sample).is_ok() {
            return true;
        }

        let printable = sample
            .iter()
            .filter(|&&b| b == b'\n' || b == b'\r' || b == b'\t' || (0x20..=0x7E).contains(&b))
            .count();

        printable * 100 / sample_len >= 90
    }

    /// Create a new ResponseData with computed hash
    pub fn new(
        status_code: u16,
        status_text: String,
        headers: HashMap<String, String>,
        body: String,
        body_bytes: Vec<u8>,
        body_size_bytes: usize,
        duration_ms: u64,
        content_type: Option<String>,
    ) -> Self {
        let mut response = Self {
            status_code,
            status_text,
            headers,
            payload: ResponsePayload {
                body: Arc::from(body),
                body_bytes: Bytes::from(body_bytes),
                ..ResponsePayload::default()
            },
            body_size_bytes,
            duration_ms,
            content_type,
        };
        response.compact_storage();
        // HTTP responses are constructed on the Tokio worker, so eagerly populate the
        // expensive JSON display cache before handing the payload to GPUI.
        if response.is_json() {
            let _ = response.formatted_body();
        }
        response
    }

    pub fn from_bytes(
        status_code: u16,
        status_text: String,
        headers: HashMap<String, String>,
        body_bytes: Bytes,
        duration_ms: u64,
        content_type: Option<String>,
    ) -> Self {
        let body_size_bytes = body_bytes.len();
        let should_decode = Self::should_eagerly_decode_body(content_type.as_deref(), &body_bytes);

        let (body, stored_body_bytes) = if should_decode {
            (
                String::from_utf8_lossy(&body_bytes).into_owned(),
                Bytes::new(),
            )
        } else {
            (String::new(), body_bytes)
        };

        let mut response = Self {
            status_code,
            status_text,
            headers,
            payload: ResponsePayload {
                body: Arc::from(body),
                body_bytes: stored_body_bytes,
                ..ResponsePayload::default()
            },
            body_size_bytes,
            duration_ms,
            content_type,
        };
        response.compact_storage();
        response
    }

    /// Compute hash for content change detection
    fn compute_hash(content: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn classify_content(content_type: Option<&str>, body_bytes: &[u8]) -> ContentCategory {
        let ct = content_type
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();

        if ct.contains("application/json") || ct.contains("text/json") || ct.ends_with("+json") {
            ContentCategory::Json
        } else if ct.contains("text/html") {
            ContentCategory::Html
        } else if ct.contains("application/xml") || ct.contains("text/xml") || ct.ends_with("+xml")
        {
            ContentCategory::Xml
        } else if ct.starts_with("image/") {
            ContentCategory::Image
        } else if ct.starts_with("audio/") {
            ContentCategory::Audio
        } else if Self::looks_like_audio(body_bytes) {
            ContentCategory::Audio
        } else if Self::looks_like_image(body_bytes) {
            ContentCategory::Image
        } else if ct.starts_with("text/")
            || ct.contains("javascript")
            || ct.contains("css")
            || ct.is_empty()
        {
            if ct.is_empty() && !body_bytes.is_empty() && !Self::looks_like_text(body_bytes) {
                ContentCategory::Binary
            } else {
                ContentCategory::Text
            }
        } else {
            ContentCategory::Binary
        }
    }

    pub fn should_eagerly_decode_body(content_type: Option<&str>, body_bytes: &[u8]) -> bool {
        matches!(
            Self::classify_content(content_type, body_bytes),
            ContentCategory::Json
                | ContentCategory::Html
                | ContentCategory::Xml
                | ContentCategory::Text
        )
    }

    /// Get the body hash for efficient change detection
    pub fn body_hash(&self) -> u64 {
        self.payload.body_hash
    }

    pub fn body(&self) -> &str {
        &self.payload.body
    }

    pub fn body_bytes(&self) -> &Bytes {
        &self.payload.body_bytes
    }

    /// Drop redundant raw bytes for text-like responses and recompute cached metadata.
    pub fn compact_storage(&mut self) -> bool {
        let had_body_bytes = !self.payload.body_bytes.is_empty();

        if had_body_bytes
            && !self.payload.body.is_empty()
            && matches!(
                Self::classify_content(self.content_type.as_deref(), &self.payload.body_bytes),
                ContentCategory::Json
                    | ContentCategory::Html
                    | ContentCategory::Xml
                    | ContentCategory::Text
            )
        {
            self.payload.body_bytes = Bytes::new();
        }

        if self.body_size_bytes == 0 {
            self.body_size_bytes = if self.payload.body_bytes.is_empty() {
                self.payload.body.len()
            } else {
                self.payload.body_bytes.len()
            };
        }

        self.payload.cached_formatted_body = None;
        self.payload.cached_raw_body = None;
        self.payload.body_hash =
            Self::compute_stored_hash(&self.payload.body, &self.payload.body_bytes);

        had_body_bytes && self.payload.body_bytes.is_empty()
    }

    /// Detect content category from content-type header
    pub fn content_category(&self) -> ContentCategory {
        Self::classify_content(self.content_type.as_deref(), &self.payload.body_bytes)
    }

    fn raw_body_arc(&mut self) -> Arc<str> {
        if !self.payload.body.is_empty() || self.payload.body_bytes.is_empty() {
            return self.payload.body.clone();
        }
        if let Some(raw) = &self.payload.cached_raw_body {
            return raw.clone();
        }
        let raw: Arc<str> =
            Arc::from(String::from_utf8_lossy(&self.payload.body_bytes).into_owned());
        self.payload.cached_raw_body = Some(raw.clone());
        raw
    }

    /// Check if response is JSON based on content-type
    pub fn is_json(&self) -> bool {
        self.content_category() == ContentCategory::Json
    }

    /// Check if response is an image
    pub fn is_image(&self) -> bool {
        self.content_category() == ContentCategory::Image
    }

    /// Get image MIME type if this is an image
    pub fn image_mime_type(&self) -> Option<&str> {
        if self.is_image() {
            self.content_type.as_deref()
        } else {
            None
        }
    }

    /// Get formatted body if JSON, otherwise raw
    pub fn formatted_body(&mut self) -> Arc<str> {
        if let Some(ref cached) = self.payload.cached_formatted_body {
            return cached.clone();
        }

        let body = self.raw_body_arc();
        let formatted = if self.is_json() {
            // Try to pretty-print JSON
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(value) => serde_json::to_string_pretty(&value)
                    .map(Arc::<str>::from)
                    .unwrap_or_else(|_| body.clone()),
                Err(_) => body.clone(),
            }
        } else {
            body.clone()
        };

        self.payload.cached_formatted_body = Some(formatted.clone());
        formatted
    }

    pub fn raw_body(&mut self) -> Arc<str> {
        self.raw_body_arc()
    }

    /// Get status category (1xx, 2xx, etc.)
    pub fn status_category(&self) -> u8 {
        (self.status_code / 100) as u8
    }

    /// Human-readable size
    pub fn formatted_size(&self) -> String {
        let bytes = self.body_size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    /// Human-readable duration
    pub fn formatted_duration(&self) -> String {
        if self.duration_ms < 1000 {
            format!("{} ms", self.duration_ms)
        } else {
            format!("{:.2} s", self.duration_ms as f64 / 1000.0)
        }
    }
}

/// ResponseEntity - GPUI Entity wrapper
pub struct ResponseEntity {
    pub state: ResponseState,
    pub data: Option<ResponseData>,
}

#[allow(dead_code)]
impl ResponseEntity {
    pub fn new() -> Self {
        Self {
            state: ResponseState::Idle,
            data: None,
        }
    }

    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.state = ResponseState::Loading;
        self.data = None;
        cx.emit(ResponseEvent::Loading);
        cx.notify();
    }

    pub fn set_response(&mut self, data: ResponseData, cx: &mut Context<Self>) {
        self.state = ResponseState::Success;
        self.data = Some(data);
        cx.emit(ResponseEvent::Received);
        cx.notify();
    }

    /// Alias for set_response
    pub fn set_success(&mut self, data: ResponseData, cx: &mut Context<Self>) {
        self.set_response(data, cx);
    }

    pub fn set_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.state = ResponseState::Error(error.clone());
        self.data = None;
        cx.emit(ResponseEvent::Error(error));
        cx.notify();
    }

    pub fn set_cancelled(&mut self, cx: &mut Context<Self>) {
        self.state = ResponseState::Cancelled;
        self.data = None;
        cx.emit(ResponseEvent::Cleared);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.state = ResponseState::Idle;
        self.data = None;
        cx.emit(ResponseEvent::Cleared);
        cx.notify();
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, ResponseState::Loading)
    }

    pub fn is_success(&self) -> bool {
        matches!(self.state, ResponseState::Success)
    }

    pub fn is_error(&self) -> bool {
        matches!(self.state, ResponseState::Error(_))
    }

    pub fn error_message(&self) -> Option<&str> {
        match &self.state {
            ResponseState::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

impl EventEmitter<ResponseEvent> for ResponseEntity {}

#[cfg(test)]
mod tests {
    use super::{ContentCategory, ResponseData};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn response_with(content_type: Option<&str>, body_bytes: Vec<u8>) -> ResponseData {
        let body = String::from_utf8_lossy(&body_bytes).to_string();
        ResponseData::new(
            200,
            "OK".to_string(),
            HashMap::new(),
            body,
            body_bytes,
            0,
            0,
            content_type.map(str::to_string),
        )
    }

    #[test]
    fn classifies_audio_from_content_type() {
        let data = response_with(Some("audio/mpeg"), b"not-important".to_vec());
        assert_eq!(data.content_category(), ContentCategory::Audio);
    }

    #[test]
    fn classifies_audio_from_magic_bytes_when_header_is_generic() {
        let data = response_with(
            Some("application/octet-stream"),
            b"ID3\x04\x00\x00\x00\x00\x00\x21".to_vec(),
        );
        assert_eq!(data.content_category(), ContentCategory::Audio);
    }

    #[test]
    fn classifies_unknown_non_text_without_header_as_binary() {
        let data = response_with(None, vec![0x00, 0x9F, 0x92, 0x00, 0xFF]);
        assert_eq!(data.content_category(), ContentCategory::Binary);
    }

    #[test]
    fn compacts_text_responses_by_dropping_duplicate_raw_bytes() {
        let data = response_with(Some("application/json"), br#"{"ok":true}"#.to_vec());
        assert!(data.body_bytes().is_empty());
        assert_eq!(data.body(), r#"{"ok":true}"#);
        assert_ne!(data.body_hash(), 0);
    }

    #[test]
    fn keeps_raw_bytes_for_binary_responses() {
        let png = b"\x89PNG\r\n\x1A\n\x00\x00\x00\x0DIHDR".to_vec();
        let data = response_with(Some("image/png"), png.clone());
        assert_eq!(data.content_category(), ContentCategory::Image);
        assert_eq!(data.body_bytes().as_ref(), png);
    }

    #[test]
    fn decodes_legacy_text_history_shape() {
        let mut data: ResponseData = serde_json::from_value(serde_json::json!({
            "status_code": 200,
            "status_text": "OK",
            "headers": {},
            "body": "{\"ok\":true}",
            "body_bytes": null,
            "body_size_bytes": 11,
            "duration_ms": 4,
            "content_type": "application/json"
        }))
        .expect("legacy response should deserialize");
        data.compact_storage();
        assert_eq!(data.body(), "{\"ok\":true}");
        assert!(data.body_bytes().is_empty());
    }

    #[test]
    fn decodes_legacy_binary_history_shape() {
        let bytes = b"\x89PNG\r\n\x1a\n";
        let mut data: ResponseData = serde_json::from_value(serde_json::json!({
            "status_code": 200,
            "status_text": "OK",
            "headers": {},
            "body": "",
            "body_bytes": STANDARD.encode(bytes),
            "body_size_bytes": bytes.len(),
            "duration_ms": 4,
            "content_type": "image/png"
        }))
        .expect("legacy response should deserialize");
        data.compact_storage();
        assert_eq!(data.body_bytes().as_ref(), bytes);
    }

    #[test]
    fn response_payload_clones_are_shallow() {
        let data = response_with(Some("image/png"), b"\x89PNG\r\n\x1a\nshared".to_vec());
        let clone = data.clone();
        assert!(Arc::ptr_eq(&data.payload.body, &clone.payload.body));
        assert_eq!(data.body_bytes().as_ptr(), clone.body_bytes().as_ptr());
    }

    #[test]
    fn raw_and_formatted_caches_are_independent() {
        let mut data = response_with(Some("application/json"), br#"{"a":1}"#.to_vec());
        let raw = data.raw_body();
        let formatted = data.formatted_body();
        assert_eq!(&*raw, r#"{"a":1}"#);
        assert_eq!(&*formatted, "{\n  \"a\": 1\n}");
        assert_eq!(&*data.raw_body(), r#"{"a":1}"#);
    }
}
