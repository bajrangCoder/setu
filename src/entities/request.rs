use gpui::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// HTTP Methods supported by the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

#[allow(dead_code)]
impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }

    pub fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Delete,
            HttpMethod::Patch,
            HttpMethod::Head,
            HttpMethod::Options,
        ]
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single header key-value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl Header {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
        }
    }
}

/// Request body content
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RequestBody {
    #[default]
    None,
    Text(String),
    Json(String),
    FormData(HashMap<String, String>),
}

#[allow(dead_code)]
impl RequestBody {
    pub fn is_empty(&self) -> bool {
        match self {
            RequestBody::None => true,
            RequestBody::Text(s) | RequestBody::Json(s) => s.is_empty(),
            RequestBody::FormData(m) => m.is_empty(),
        }
    }
}

/// Events emitted by RequestEntity
#[derive(Debug, Clone)]
pub enum RequestEvent {
    UrlChanged,
    MethodChanged,
    HeadersChanged,
    BodyChanged,
    Sending,
    Completed,
}

/// The main request data entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestData {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub method: HttpMethod,
    pub headers: Vec<Header>,
    pub body: RequestBody,
    #[serde(skip)]
    pub is_sending: bool,
}

impl Default for RequestData {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::from("New Request"),
            url: String::new(),
            method: HttpMethod::Get,
            headers: vec![Header::new("Content-Type", "application/json")],
            body: RequestBody::None,
            is_sending: false,
        }
    }
}

/// RequestEntity - GPUI Entity wrapper
pub struct RequestEntity {
    pub data: RequestData,
}

#[allow(dead_code)]
impl RequestEntity {
    pub fn new() -> Self {
        Self {
            data: RequestData::default(),
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.data.url = url.into();
        self
    }

    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.data.method = method;
        self
    }

    pub fn set_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.data.url = url;
        cx.emit(RequestEvent::UrlChanged);
        cx.notify();
    }

    pub fn set_method(&mut self, method: HttpMethod, cx: &mut Context<Self>) {
        self.data.method = method;
        cx.emit(RequestEvent::MethodChanged);
        cx.notify();
    }

    pub fn set_body(&mut self, body: RequestBody, cx: &mut Context<Self>) {
        self.data.body = body;
        cx.emit(RequestEvent::BodyChanged);
        cx.notify();
    }

    pub fn add_header(&mut self, header: Header, cx: &mut Context<Self>) {
        self.data.headers.push(header);
        cx.emit(RequestEvent::HeadersChanged);
        cx.notify();
    }

    pub fn remove_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.headers.len() {
            self.data.headers.remove(index);
            cx.emit(RequestEvent::HeadersChanged);
            cx.notify();
        }
    }

    pub fn set_sending(&mut self, sending: bool, cx: &mut Context<Self>) {
        self.data.is_sending = sending;
        if sending {
            cx.emit(RequestEvent::Sending);
        } else {
            cx.emit(RequestEvent::Completed);
        }
        cx.notify();
    }

    pub fn url(&self) -> &str {
        &self.data.url
    }

    pub fn method(&self) -> HttpMethod {
        self.data.method
    }

    pub fn headers(&self) -> &[Header] {
        &self.data.headers
    }

    pub fn body(&self) -> &RequestBody {
        &self.data.body
    }

    pub fn is_sending(&self) -> bool {
        self.data.is_sending
    }
}

impl EventEmitter<RequestEvent> for RequestEntity {}
