use base64::{engine::general_purpose::STANDARD, Engine};
use gpui::{Context, EventEmitter};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

fn serialize_bytes_as_base64<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if bytes.is_empty() {
        serializer.serialize_none()
    } else {
        serializer.serialize_some(&STANDARD.encode(bytes))
    }
}

fn deserialize_bytes_from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => STANDARD.decode(&s).map_err(serde::de::Error::custom),
        None => Ok(Vec::new()),
    }
}

/// Response state machine
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ResponseState {
    #[default]
    Idle,
    Loading,
    Success,
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

/// HTTP Response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    /// Text body (for text-based responses)
    pub body: String,
    /// Raw bytes body (for binary responses like images)
    #[serde(
        serialize_with = "serialize_bytes_as_base64",
        deserialize_with = "deserialize_bytes_from_base64",
        default
    )]
    pub body_bytes: Vec<u8>,
    pub body_size_bytes: usize,
    pub duration_ms: u64,
    pub content_type: Option<String>,
    /// Cached formatted body
    #[serde(skip)]
    cached_formatted_body: Option<Arc<String>>,
    /// Hash of body content for efficient change detection
    #[serde(skip)]
    body_hash: u64,
}

impl Default for ResponseData {
    fn default() -> Self {
        Self {
            status_code: 0,
            status_text: String::new(),
            headers: HashMap::new(),
            body: String::new(),
            body_bytes: Vec::new(),
            body_size_bytes: 0,
            duration_ms: 0,
            content_type: None,
            cached_formatted_body: None,
            body_hash: 0,
        }
    }
}

#[allow(dead_code)]
impl ResponseData {
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
        let body_hash = Self::compute_hash(&body);
        Self {
            status_code,
            status_text,
            headers,
            body,
            body_bytes,
            body_size_bytes,
            duration_ms,
            content_type,
            cached_formatted_body: None,
            body_hash,
        }
    }

    /// Compute hash for content change detection
    fn compute_hash(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get the body hash for efficient change detection
    pub fn body_hash(&self) -> u64 {
        self.body_hash
    }

    /// Detect content category from content-type header
    pub fn content_category(&self) -> ContentCategory {
        let ct = self
            .content_type
            .as_deref()
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
        } else if Self::looks_like_audio(&self.body_bytes) {
            ContentCategory::Audio
        } else if Self::looks_like_image(&self.body_bytes) {
            ContentCategory::Image
        } else if ct.starts_with("text/")
            || ct.contains("javascript")
            || ct.contains("css")
            || ct.is_empty()
        {
            if ct.is_empty()
                && !self.body_bytes.is_empty()
                && !Self::looks_like_text(&self.body_bytes)
            {
                ContentCategory::Binary
            } else {
                ContentCategory::Text
            }
        } else {
            ContentCategory::Binary
        }
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
    pub fn formatted_body(&mut self) -> Arc<String> {
        if let Some(ref cached) = self.cached_formatted_body {
            return cached.clone();
        }

        let formatted = if self.is_json() {
            // Try to pretty-print JSON
            match serde_json::from_str::<serde_json::Value>(&self.body) {
                Ok(value) => {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| self.body.clone())
                }
                Err(_) => self.body.clone(),
            }
        } else {
            self.body.clone()
        };

        let arc = Arc::new(formatted);
        self.cached_formatted_body = Some(arc.clone());
        arc
    }

    /// Get formatted body without requiring mutable access (returns cached or raw)
    pub fn formatted_body_ref(&self) -> &str {
        if let Some(ref cached) = self.cached_formatted_body {
            cached.as_str()
        } else {
            &self.body
        }
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

    pub fn set_response(&mut self, mut data: ResponseData, cx: &mut Context<Self>) {
        let _ = data.formatted_body();

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
    use std::collections::HashMap;

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
}
