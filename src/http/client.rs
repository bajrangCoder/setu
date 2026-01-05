use crate::entities::{Header, HttpMethod, RequestBody, ResponseData};
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;

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

    pub fn execute_sync(
        &self,
        method: HttpMethod,
        url: &str,
        headers: &[Header],
        body: &RequestBody,
    ) -> Result<ResponseData> {
        // Run the async request on the Tokio runtime
        self.runtime
            .block_on(self.execute_async(method, url, headers, body))
    }

    /// Internal async execution
    async fn execute_async(
        &self,
        method: HttpMethod,
        url: &str,
        headers: &[Header],
        body: &RequestBody,
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
            HttpMethod::Get => self.client.get(&url),
            HttpMethod::Post => self.client.post(&url),
            HttpMethod::Put => self.client.put(&url),
            HttpMethod::Delete => self.client.delete(&url),
            HttpMethod::Patch => self.client.patch(&url),
            HttpMethod::Head => self.client.head(&url),
            HttpMethod::Options => self.client.request(reqwest::Method::OPTIONS, &url),
        };

        // Add headers
        for header in headers.iter().filter(|h| h.enabled) {
            request = request.header(&header.key, &header.value);
        }

        // Add body
        request = match body {
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
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}
