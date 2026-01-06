use crate::entities::{Header, HttpMethod, RequestBody, ResponseData};
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

/// HTTP Client wrapper for making requests
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    runtime: Arc<Runtime>,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder().user_agent("Setu/0.1.0").build()?;

        let runtime =
            Runtime::new().map_err(|e| anyhow!("Failed to create Tokio runtime: {}", e))?;

        Ok(Self {
            client,
            runtime: Arc::new(runtime),
        })
    }

    /// Spawn an HTTP request on the Tokio runtime and return a receiver for the result.
    /// This allows GPUI's async executor to await the result without being in a Tokio context.
    pub fn spawn_request(
        &self,
        method: HttpMethod,
        url: String,
        headers: Vec<Header>,
        body: RequestBody,
    ) -> oneshot::Receiver<Result<ResponseData>> {
        let (tx, rx) = oneshot::channel();
        let client = self.client.clone();

        self.runtime.spawn(async move {
            let result = execute_request(client, method, url, headers, body).await;
            let _ = tx.send(result);
        });

        rx
    }
}

/// Internal function to execute the HTTP request
async fn execute_request(
    client: Client,
    method: HttpMethod,
    url: String,
    headers: Vec<Header>,
    body: RequestBody,
) -> Result<ResponseData> {
    // Validate URL
    if url.is_empty() {
        return Err(anyhow!("URL cannot be empty"));
    }

    // Ensure URL has a scheme
    let url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    let start = Instant::now();

    // Build request
    let mut request = match method {
        HttpMethod::Get => client.get(&url),
        HttpMethod::Post => client.post(&url),
        HttpMethod::Put => client.put(&url),
        HttpMethod::Delete => client.delete(&url),
        HttpMethod::Patch => client.patch(&url),
        HttpMethod::Head => client.head(&url),
        HttpMethod::Options => client.request(reqwest::Method::OPTIONS, &url),
    };

    // Add headers
    for header in headers.iter().filter(|h| h.enabled) {
        request = request.header(&header.key, &header.value);
    }

    // Add body
    request = match &body {
        RequestBody::None => request,
        RequestBody::Text(text) => request.body(text.clone()),
        RequestBody::Json(json) => {
            let normalized_json = match serde_json::from_str::<serde_json::Value>(json) {
                Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| json.clone()),
                Err(e) => {
                    log::warn!("Invalid JSON, sending as-is: {}", e);
                    json.clone()
                }
            };
            request
                .header("Content-Type", "application/json")
                .body(normalized_json)
        }
        // For form data, serialize as JSON for now
        RequestBody::FormData(data) => request
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(data).unwrap_or_default()),
    };

    // Execute request
    let response = request.send().await?;
    let duration = start.elapsed();

    // Extract response data
    let status_code = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    // Get content-type before consuming response
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Convert headers
    let mut response_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            response_headers.insert(key.to_string(), v.to_string());
        }
    }

    // Get body
    let body = response.text().await.unwrap_or_default();
    let body_size_bytes = body.len();

    Ok(ResponseData {
        status_code,
        status_text,
        headers: response_headers,
        body,
        body_size_bytes,
        duration_ms: duration.as_millis() as u64,
        content_type,
    })
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}
