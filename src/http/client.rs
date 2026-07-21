use crate::entities::{Header, HttpMethod, RequestBody, ResponseData};
use crate::utils::shared_tokio_runtime;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// HTTP Client wrapper for making requests
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    runtime: Arc<tokio::runtime::Runtime>,
}

/// Handle for canceling an in-flight HTTP request.
pub struct InFlightRequest {
    task: Option<JoinHandle<()>>,
}

impl InFlightRequest {
    pub fn cancel(&mut self) -> bool {
        if let Some(task) = self.task.take() {
            task.abort();
            true
        } else {
            false
        }
    }
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder().user_agent("Setu/0.1.0").build()?;

        Ok(Self {
            client,
            runtime: shared_tokio_runtime(),
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
    ) -> (oneshot::Receiver<Result<ResponseData>>, InFlightRequest) {
        let (tx, rx) = oneshot::channel();
        let client = self.client.clone();

        let task = self.runtime.spawn(async move {
            let result = execute_request(client, method, url, headers, body).await;
            let _ = tx.send(result);
        });

        (rx, InFlightRequest { task: Some(task) })
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

    // Check if this is a multipart request
    let is_multipart = matches!(body, RequestBody::MultipartFormData(_));

    // Add headers (skip Content-Type for multipart - reqwest sets it with boundary)
    for header in headers.iter().filter(|h| h.enabled) {
        if is_multipart && header.key.to_lowercase() == "content-type" {
            continue;
        }
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
        // For form data, send as application/x-www-form-urlencoded
        RequestBody::FormData(data) => request.form(data),
        // For multipart form data, use reqwest's multipart support
        RequestBody::MultipartFormData(fields) => {
            let mut form = reqwest::multipart::Form::new();

            for field in fields {
                if let Some(ref file_path) = field.file_path {
                    let path = std::path::Path::new(file_path);
                    match tokio::fs::read(path).await {
                        Ok(file_bytes) => {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_string();

                            let mime_type = mime_guess::from_path(path)
                                .first_or_octet_stream()
                                .to_string();

                            let part = reqwest::multipart::Part::bytes(file_bytes)
                                .file_name(file_name)
                                .mime_str(&mime_type)
                                .unwrap_or_else(|_| reqwest::multipart::Part::bytes(vec![]));

                            form = form.part(field.key.clone(), part);
                        }
                        Err(e) => {
                            log::error!("Failed to read file {}: {}", file_path, e);
                        }
                    }
                } else {
                    form = form.text(field.key.clone(), field.value.clone());
                }
            }

            request.multipart(form)
        }
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

    // Get body as bytes first
    let body_bytes = response.bytes().await?;

    Ok(ResponseData::from_bytes(
        status_code,
        status_text,
        response_headers,
        body_bytes,
        duration.as_millis() as u64,
        content_type,
    ))
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::{HttpClient, execute_request};
    use crate::entities::{ContentCategory, HttpMethod, MultipartField, RequestBody};
    use crate::utils::shared_tokio_runtime;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        let mut expected_len = None;
        loop {
            let read = stream.read(&mut buffer).unwrap_or(0);
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if expected_len.is_none()
                && let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n")
            {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                expected_len = Some(header_end + 4 + content_length);
            }
            if expected_len.is_some_and(|length| request.len() >= length) {
                break;
            }
        }
        request
    }

    fn spawn_server(
        status: &'static str,
        content_type: &'static str,
        body: Vec<u8>,
    ) -> (String, mpsc::Receiver<Vec<u8>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_tx, request_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let _ = request_tx.send(request);
            let headers = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(headers.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        });
        (format!("http://{address}"), request_rx, handle)
    }

    #[test]
    fn sends_url_encoded_forms_with_reqwest_form_api() {
        let (url, request_rx, server) = spawn_server("200 OK", "text/plain", b"ok".to_vec());
        let body = RequestBody::FormData(HashMap::from([(
            "display name".to_string(),
            "Setu User".to_string(),
        )]));
        let response = shared_tokio_runtime()
            .block_on(execute_request(
                reqwest::Client::new(),
                HttpMethod::Post,
                url,
                Vec::new(),
                body,
            ))
            .unwrap();
        let request = String::from_utf8_lossy(&request_rx.recv().unwrap()).into_owned();
        server.join().unwrap();

        assert_eq!(response.status_code, 200);
        assert!(request.contains("content-type: application/x-www-form-urlencoded"));
        assert!(request.ends_with("display+name=Setu+User"));
    }

    #[test]
    fn sends_multipart_files_without_losing_bytes() {
        let path = std::env::temp_dir().join(format!("setu-{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"multipart-payload").unwrap();
        let (url, request_rx, server) = spawn_server("200 OK", "text/plain", b"ok".to_vec());
        let body = RequestBody::MultipartFormData(vec![MultipartField::file(
            "upload",
            path.to_string_lossy(),
        )]);
        shared_tokio_runtime()
            .block_on(execute_request(
                reqwest::Client::new(),
                HttpMethod::Post,
                url,
                Vec::new(),
                body,
            ))
            .unwrap();
        let request = request_rx.recv().unwrap();
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(
            request
                .windows(17)
                .any(|window| window == b"multipart-payload")
        );
        assert!(String::from_utf8_lossy(&request).contains("name=\"upload\""));
    }

    #[test]
    fn preserves_binary_image_and_audio_payloads() {
        for (content_type, body, category) in [
            (
                "image/png",
                b"\x89PNG\r\n\x1a\nnetwork".to_vec(),
                ContentCategory::Image,
            ),
            (
                "audio/mpeg",
                b"ID3\x04\x00\x00network".to_vec(),
                ContentCategory::Audio,
            ),
        ] {
            let (url, _request_rx, server) = spawn_server("200 OK", content_type, body.clone());
            let response = shared_tokio_runtime()
                .block_on(execute_request(
                    reqwest::Client::new(),
                    HttpMethod::Get,
                    url,
                    Vec::new(),
                    RequestBody::None,
                ))
                .unwrap();
            server.join().unwrap();
            assert_eq!(response.content_category(), category);
            assert_eq!(response.body_bytes().as_ref(), body);
        }
    }

    #[test]
    fn rejects_cancelled_http_results() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            accepted_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(50));
        });
        let client = HttpClient::new().unwrap();
        let (result, mut in_flight) =
            client.spawn_request(HttpMethod::Get, url, Vec::new(), RequestBody::None);
        accepted_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(in_flight.cancel());
        assert!(shared_tokio_runtime().block_on(result).is_err());
        server.join().unwrap();
    }

    #[test]
    fn handles_full_ten_megabyte_json_response() {
        let body = format!("{{\"payload\":\"{}\"}}", "x".repeat(10 * 1024 * 1024)).into_bytes();
        let expected_len = body.len();
        let (url, _request_rx, server) = spawn_server("200 OK", "application/json", body);
        let response = shared_tokio_runtime()
            .block_on(execute_request(
                reqwest::Client::new(),
                HttpMethod::Get,
                url,
                Vec::new(),
                RequestBody::None,
            ))
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.body_size_bytes, expected_len);
        assert_eq!(response.body().len(), expected_len);
        assert!(response.body_bytes().is_empty());
    }

    #[test]
    fn follows_redirects_with_the_configured_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let _ = read_request(&mut first);
            first
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .unwrap();

            let (mut second, _) = listener.accept().unwrap();
            let request = read_request(&mut second);
            assert!(String::from_utf8_lossy(&request).starts_with("GET /final "));
            second
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 8\r\nConnection: close\r\n\r\nredirect",
                )
                .unwrap();
        });

        let response = shared_tokio_runtime()
            .block_on(execute_request(
                HttpClient::new().unwrap().client,
                HttpMethod::Get,
                format!("http://{address}/start"),
                Vec::new(),
                RequestBody::None,
            ))
            .unwrap();
        server.join().unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body(), "redirect");
    }
}
