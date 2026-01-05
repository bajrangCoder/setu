use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response state machine
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ResponseState {
    #[default]
    Idle,
    Loading,
    Success,
    Error(String),
}

/// Events emitted by ResponseEntity
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
    pub body: String,
    pub body_size_bytes: usize,
    pub duration_ms: u64,
    pub content_type: Option<String>,
}

impl Default for ResponseData {
    fn default() -> Self {
        Self {
            status_code: 0,
            status_text: String::new(),
            headers: HashMap::new(),
            body: String::new(),
            body_size_bytes: 0,
            duration_ms: 0,
            content_type: None,
        }
    }
}

impl ResponseData {
    /// Check if response is JSON based on content-type
    pub fn is_json(&self) -> bool {
        self.content_type
            .as_ref()
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false)
    }

    /// Get formatted body if JSON, otherwise raw
    pub fn formatted_body(&self) -> String {
        if self.is_json() {
            // Try to pretty-print JSON
            match serde_json::from_str::<serde_json::Value>(&self.body) {
                Ok(value) => {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| self.body.clone())
                }
                Err(_) => self.body.clone(),
            }
        } else {
            self.body.clone()
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
