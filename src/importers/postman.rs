use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use crate::entities::{Header, HttpMethod, MultipartField, RequestBody, RequestData};

use super::{CollectionImporter, ImportResult, ImportWarning, ImportedCollection, ImportedNode};

#[derive(Default)]
pub struct PostmanCollectionImporter;

impl CollectionImporter for PostmanCollectionImporter {
    fn provider_name(&self) -> &'static str {
        "Postman"
    }

    fn matches(&self, _path: &Path, contents: &str) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(contents) else {
            return false;
        };

        let info = value.get("info").and_then(Value::as_object);
        let has_items = value.get("item").and_then(Value::as_array).is_some();
        let schema_matches = info
            .and_then(|info| info.get("schema"))
            .and_then(Value::as_str)
            .map(|schema| schema.contains("getpostman.com") && schema.contains("collection"))
            .unwrap_or(false);
        let postman_id = info.and_then(|info| info.get("_postman_id")).is_some();

        has_items && (schema_matches || postman_id || info.is_some())
    }

    fn import(&self, path: &Path, contents: &str) -> Result<ImportResult> {
        let document: PostmanCollectionDocument = serde_json::from_str(contents)
            .map_err(|err| anyhow!("Failed to parse Postman collection JSON: {err}"))?;

        let collection_name = document
            .info
            .as_ref()
            .and_then(|info| info.name.clone())
            .or_else(|| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(str::to_string)
            })
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Imported Collection".to_string());

        let mut warnings = Vec::new();
        let collection_path = vec![collection_name.clone()];
        let inherited_auth = document
            .auth
            .and_then(|auth| resolve_auth(auth, &collection_path, &mut warnings));

        warn_on_unsupported_fields(
            &mut warnings,
            &collection_path,
            !document.event.is_empty(),
            "Collection scripts are skipped.",
        );
        warn_on_unsupported_fields(
            &mut warnings,
            &collection_path,
            !document.variable.is_empty(),
            "Collection variables are skipped.",
        );

        let nodes = document
            .item
            .into_iter()
            .filter_map(|item| {
                import_item(
                    item,
                    &collection_path,
                    inherited_auth.as_ref(),
                    &mut warnings,
                )
            })
            .collect();

        Ok(ImportResult {
            provider: self.provider_name(),
            collection: ImportedCollection {
                name: collection_name,
                nodes,
            },
            warnings,
        })
    }
}

#[derive(Debug, Deserialize)]
struct PostmanCollectionDocument {
    info: Option<PostmanInfo>,
    #[serde(default)]
    item: Vec<PostmanItem>,
    #[serde(default)]
    event: Vec<Value>,
    #[serde(default)]
    variable: Vec<Value>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
struct PostmanInfo {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostmanItem {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    item: Vec<PostmanItem>,
    #[serde(default)]
    request: Option<PostmanRequestValue>,
    #[serde(default)]
    event: Vec<Value>,
    #[serde(default)]
    response: Vec<Value>,
    #[serde(default)]
    variable: Vec<Value>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanRequestValue {
    Request(PostmanRequest),
    Url(String),
}

#[derive(Debug, Deserialize)]
struct PostmanRequest {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    header: Option<PostmanHeaders>,
    #[serde(default)]
    body: Option<PostmanBody>,
    #[serde(default)]
    url: Option<PostmanUrl>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanHeaders {
    Items(Vec<PostmanHeader>),
    Raw(String),
}

#[derive(Debug, Deserialize)]
struct PostmanHeader {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanUrl {
    String(String),
    Structured(PostmanUrlObject),
}

#[derive(Debug, Deserialize, Default)]
struct PostmanUrlObject {
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    host: Vec<String>,
    #[serde(default)]
    path: Vec<String>,
    #[serde(default)]
    query: Vec<PostmanKeyValue>,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanBody {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    urlencoded: Vec<PostmanKeyValue>,
    #[serde(default)]
    formdata: Vec<PostmanFormDataField>,
    #[serde(default)]
    options: Option<PostmanBodyOptions>,
    #[serde(default)]
    graphql: Option<Value>,
    #[serde(default)]
    file: Option<PostmanFileBody>,
    #[serde(default)]
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanBodyOptions {
    #[serde(default)]
    raw: Option<PostmanRawOptions>,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanRawOptions {
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PostmanAuth {
    #[serde(rename = "type")]
    auth_type: String,
    #[serde(default)]
    apikey: Vec<PostmanAuthAttribute>,
    #[serde(default)]
    basic: Vec<PostmanAuthAttribute>,
    #[serde(default)]
    bearer: Vec<PostmanAuthAttribute>,
}

#[derive(Debug, Clone, Deserialize)]
struct PostmanAuthAttribute {
    key: String,
    #[serde(default)]
    value: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanKeyValue {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanFormDataField {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default, rename = "type")]
    field_type: Option<String>,
    #[serde(default)]
    src: Option<PostmanFileSource>,
    #[serde(default)]
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanFileSource {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize, Default)]
struct PostmanFileBody {
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Clone)]
enum ImportedAuth {
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        key: String,
        value: String,
        in_header: bool,
    },
}

fn import_item(
    item: PostmanItem,
    parent_path: &[String],
    inherited_auth: Option<&ImportedAuth>,
    warnings: &mut Vec<ImportWarning>,
) -> Option<ImportedNode> {
    let label = item
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Untitled".to_string());
    let mut item_path = parent_path.to_vec();
    item_path.push(label.clone());
    let effective_auth = match item.auth {
        Some(auth) => resolve_auth(auth, &item_path, warnings),
        None => inherited_auth.cloned(),
    };

    warn_on_unsupported_fields(
        warnings,
        &item_path,
        !item.event.is_empty(),
        "Item scripts are skipped.",
    );
    warn_on_unsupported_fields(
        warnings,
        &item_path,
        !item.response.is_empty(),
        "Saved examples are skipped.",
    );
    warn_on_unsupported_fields(
        warnings,
        &item_path,
        !item.variable.is_empty(),
        "Item variables are skipped.",
    );

    if !item.item.is_empty() {
        if item.request.is_some() {
            warnings.push(ImportWarning::new(
                Some(path_label(&item_path)),
                "This item contains both nested folders and a request. The nested tree was kept and the request was skipped.",
            ));
        }

        let children = item
            .item
            .into_iter()
            .filter_map(|child| import_item(child, &item_path, effective_auth.as_ref(), warnings))
            .collect();

        return Some(ImportedNode::Folder {
            name: label,
            children,
        });
    }

    let request = match item.request {
        Some(PostmanRequestValue::Request(request)) => request,
        Some(PostmanRequestValue::Url(raw_url)) => PostmanRequest {
            method: Some("GET".to_string()),
            header: None,
            body: None,
            url: Some(PostmanUrl::String(raw_url)),
            auth: None,
        },
        None => {
            warnings.push(ImportWarning::new(
                Some(path_label(&item_path)),
                "This Postman item did not contain a request and was skipped.",
            ));
            return None;
        }
    };

    import_request(
        label,
        request,
        &item_path,
        effective_auth.as_ref(),
        warnings,
    )
    .map(|request| ImportedNode::Request { request })
}

fn import_request(
    name: String,
    request: PostmanRequest,
    path: &[String],
    inherited_auth: Option<&ImportedAuth>,
    warnings: &mut Vec<ImportWarning>,
) -> Option<RequestData> {
    let effective_auth = match request.auth {
        Some(auth) => resolve_auth(auth, path, warnings),
        None => inherited_auth.cloned(),
    };

    let method = map_method(request.method.as_deref(), path, warnings)?;
    let mut url = match request.url {
        Some(url) => map_url(url),
        None => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                "Request URL is missing, so the request was skipped.",
            ));
            return None;
        }
    };

    let mut headers = map_headers(request.header, path, warnings);
    apply_auth(
        &mut url,
        &mut headers,
        effective_auth.as_ref(),
        path,
        warnings,
    );
    let body = map_body(request.body, &headers, path, warnings);

    Some(RequestData {
        id: Uuid::new_v4(),
        name,
        url,
        method,
        headers,
        body,
        is_sending: false,
    })
}

fn map_method(
    method: Option<&str>,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Option<HttpMethod> {
    match method.unwrap_or("GET").to_ascii_uppercase().as_str() {
        "GET" => Some(HttpMethod::Get),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        "DELETE" => Some(HttpMethod::Delete),
        "PATCH" => Some(HttpMethod::Patch),
        "HEAD" => Some(HttpMethod::Head),
        "OPTIONS" => Some(HttpMethod::Options),
        unsupported => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                format!(
                    "HTTP method `{unsupported}` is not supported in Setu yet. The request was skipped."
                ),
            ));
            None
        }
    }
}

fn map_headers(
    headers: Option<PostmanHeaders>,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Vec<Header> {
    match headers {
        Some(PostmanHeaders::Items(items)) => items
            .into_iter()
            .filter_map(|header| {
                let key = header.key?.trim().to_string();
                if key.is_empty() {
                    return None;
                }

                Some(Header {
                    key,
                    value: header.value.unwrap_or_default(),
                    enabled: !header.disabled.unwrap_or(false),
                })
            })
            .collect(),
        Some(PostmanHeaders::Raw(raw_headers)) => parse_raw_headers(&raw_headers, path, warnings),
        None => Vec::new(),
    }
}

fn map_url(url: PostmanUrl) -> String {
    match url {
        PostmanUrl::String(raw) => raw,
        PostmanUrl::Structured(url) => {
            if let Some(raw) = url.raw.filter(|raw| !raw.trim().is_empty()) {
                return raw;
            }

            let mut built = String::new();
            if let Some(protocol) = url.protocol.filter(|value| !value.is_empty()) {
                built.push_str(&protocol);
                built.push_str("://");
            }

            if !url.host.is_empty() {
                built.push_str(&url.host.join("."));
            }

            if !url.path.is_empty() {
                if !built.ends_with('/') && !built.is_empty() {
                    built.push('/');
                }
                built.push_str(&url.path.join("/"));
            }

            let query = build_query(&url.query);
            if !query.is_empty() {
                built.push('?');
                built.push_str(&query);
            }

            built
        }
    }
}

fn build_query(query: &[PostmanKeyValue]) -> String {
    query
        .iter()
        .filter(|entry| !entry.disabled.unwrap_or(false))
        .filter_map(|entry| {
            let key = entry.key.as_deref()?.trim();
            if key.is_empty() {
                return None;
            }

            let value = entry.value.as_deref().unwrap_or("");
            Some(format!("{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn map_body(
    body: Option<PostmanBody>,
    headers: &[Header],
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> RequestBody {
    let Some(body) = body else {
        return RequestBody::None;
    };

    if body.disabled.unwrap_or(false) {
        return RequestBody::None;
    }

    match body.mode.as_deref().unwrap_or("raw") {
        "raw" => {
            let raw = body.raw.unwrap_or_default();
            let language = body
                .options
                .as_ref()
                .and_then(|options| options.raw.as_ref())
                .and_then(|raw| raw.language.as_deref());

            infer_text_body(raw, headers, language, None)
        }
        "urlencoded" => {
            let mut data = HashMap::new();
            for entry in body.urlencoded {
                if entry.disabled.unwrap_or(false) {
                    continue;
                }

                let Some(key) = entry.key.filter(|key| !key.trim().is_empty()) else {
                    continue;
                };
                let value = entry.value.unwrap_or_default();

                if data.contains_key(&key) {
                    warnings.push(ImportWarning::new(
                        Some(path_label(path)),
                        format!(
                            "Duplicate x-www-form-urlencoded key `{key}` was collapsed to the last value."
                        ),
                    ));
                }
                data.insert(key, value);
            }
            RequestBody::FormData(data)
        }
        "formdata" => {
            let mut fields = Vec::new();
            for entry in body.formdata {
                if entry.disabled.unwrap_or(false) {
                    continue;
                }

                let Some(key) = entry.key.filter(|key| !key.trim().is_empty()) else {
                    continue;
                };

                match entry.field_type.as_deref().unwrap_or("text") {
                    "file" => match entry.src {
                        Some(PostmanFileSource::One(path)) => {
                            fields.push(MultipartField::file(key, path));
                        }
                        Some(PostmanFileSource::Many(paths)) => {
                            if let Some(file_path) = paths.into_iter().next() {
                                warnings.push(ImportWarning::new(
                                    Some(path_label(path)),
                                    format!(
                                        "Multiple form-data files for `{key}` were reduced to the first file."
                                    ),
                                ));
                                fields.push(MultipartField::file(key, file_path));
                            }
                        }
                        None => {
                            warnings.push(ImportWarning::new(
                                Some(path_label(path)),
                                format!(
                                    "Form-data file field `{key}` has no file source and was skipped."
                                ),
                            ));
                        }
                    },
                    _ => fields.push(MultipartField::text(key, entry.value.unwrap_or_default())),
                }
            }
            RequestBody::MultipartFormData(fields)
        }
        "graphql" => body
            .graphql
            .and_then(serialize_graphql_body)
            .map(RequestBody::Json)
            .or_else(|| {
                body.raw
                    .map(|raw| infer_text_body(raw, headers, Some("json"), None))
            })
            .unwrap_or(RequestBody::None),
        "file" => map_file_body(body.file, headers, path, warnings),
        other => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                format!("Postman body mode `{other}` is not supported yet. The body was skipped."),
            ));
            RequestBody::None
        }
    }
}

fn resolve_auth(
    auth: PostmanAuth,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Option<ImportedAuth> {
    let auth_type = auth.auth_type.to_ascii_lowercase();
    match auth_type.as_str() {
        "noauth" => None,
        "basic" => {
            let attributes = auth_attributes_to_map(auth.basic);
            let username = attributes.get("username").cloned().unwrap_or_default();
            let password = attributes.get("password").cloned().unwrap_or_default();

            if username.is_empty() && password.is_empty() {
                warnings.push(ImportWarning::new(
                    Some(path_label(path)),
                    "Basic auth was selected but no username or password was present.",
                ));
                None
            } else {
                Some(ImportedAuth::Basic { username, password })
            }
        }
        "bearer" => {
            let attributes = auth_attributes_to_map(auth.bearer);
            let token = attributes
                .get("token")
                .cloned()
                .or_else(|| attributes.get("bearer").cloned())
                .unwrap_or_default();

            if token.is_empty() {
                warnings.push(ImportWarning::new(
                    Some(path_label(path)),
                    "Bearer auth was selected but no token was present.",
                ));
                None
            } else {
                Some(ImportedAuth::Bearer { token })
            }
        }
        "apikey" => {
            let attributes = auth_attributes_to_map(auth.apikey);
            let key = attributes.get("key").cloned().unwrap_or_default();
            let value = attributes.get("value").cloned().unwrap_or_default();
            let location = attributes
                .get("in")
                .map(|location| location.to_ascii_lowercase())
                .unwrap_or_else(|| "header".to_string());

            if key.is_empty() {
                warnings.push(ImportWarning::new(
                    Some(path_label(path)),
                    "API key auth was selected but the key name was missing.",
                ));
                return None;
            }

            match location.as_str() {
                "header" | "" => Some(ImportedAuth::ApiKey {
                    key,
                    value,
                    in_header: true,
                }),
                "query" | "queryparam" => Some(ImportedAuth::ApiKey {
                    key,
                    value,
                    in_header: false,
                }),
                other => {
                    warnings.push(ImportWarning::new(
                        Some(path_label(path)),
                        format!(
                            "API key auth location `{other}` is not supported. The auth was skipped."
                        ),
                    ));
                    None
                }
            }
        }
        unsupported => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                format!("Postman auth type `{unsupported}` is not supported yet and was skipped."),
            ));
            None
        }
    }
}

fn auth_attributes_to_map(attributes: Vec<PostmanAuthAttribute>) -> HashMap<String, String> {
    attributes
        .into_iter()
        .filter_map(|attribute| {
            let key = attribute.key.trim().to_ascii_lowercase();
            if key.is_empty() {
                return None;
            }

            Some((key, value_to_string(attribute.value.unwrap_or(Value::Null))))
        })
        .collect()
}

fn value_to_string(value: Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value,
        other => other.to_string(),
    }
}

fn apply_auth(
    url: &mut String,
    headers: &mut Vec<Header>,
    auth: Option<&ImportedAuth>,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) {
    let Some(auth) = auth else {
        return;
    };

    match auth {
        ImportedAuth::Basic { username, password } => insert_auth_header(
            headers,
            "Authorization",
            format!(
                "Basic {}",
                BASE64_STANDARD.encode(format!("{username}:{password}"))
            ),
            path,
            warnings,
        ),
        ImportedAuth::Bearer { token } => insert_auth_header(
            headers,
            "Authorization",
            format!("Bearer {token}"),
            path,
            warnings,
        ),
        ImportedAuth::ApiKey {
            key,
            value,
            in_header,
        } => {
            if *in_header {
                insert_auth_header(headers, key, value.clone(), path, warnings);
            } else if url_has_query_key(url, key) {
                warnings.push(ImportWarning::new(
                    Some(path_label(path)),
                    format!(
                        "API key query parameter `{key}` was not added because the URL already defines it."
                    ),
                ));
            } else {
                append_query_param(url, key, value);
            }
        }
    }
}

fn insert_auth_header(
    headers: &mut Vec<Header>,
    key: &str,
    value: String,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) {
    if headers
        .iter()
        .any(|header| header.enabled && header.key.eq_ignore_ascii_case(key))
    {
        warnings.push(ImportWarning::new(
            Some(path_label(path)),
            format!("Auth header `{key}` was not added because the request already defines it."),
        ));
        return;
    }

    headers.push(Header {
        key: key.to_string(),
        value,
        enabled: true,
    });
}

fn parse_raw_headers(
    raw_headers: &str,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Vec<Header> {
    let mut headers = Vec::new();
    let mut invalid_lines = 0;

    for line in raw_headers.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            invalid_lines += 1;
            continue;
        };

        let key = key.trim();
        if key.is_empty() {
            invalid_lines += 1;
            continue;
        }

        headers.push(Header {
            key: key.to_string(),
            value: value.trim_start().to_string(),
            enabled: true,
        });
    }

    if invalid_lines > 0 {
        warnings.push(ImportWarning::new(
            Some(path_label(path)),
            format!(
                "{invalid_lines} raw header line{} could not be parsed and {} skipped.",
                if invalid_lines == 1 { "" } else { "s" },
                if invalid_lines == 1 { "was" } else { "were" }
            ),
        ));
    }

    headers
}

fn infer_text_body(
    raw: String,
    headers: &[Header],
    language: Option<&str>,
    source_name: Option<&str>,
) -> RequestBody {
    let language = language.unwrap_or_default().to_ascii_lowercase();
    let content_type = content_type_header(headers);
    let source_extension = source_name
        .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    if language == "json"
        || content_type.contains("application/json")
        || content_type.contains("+json")
        || source_extension == "json"
    {
        RequestBody::Json(raw)
    } else {
        RequestBody::Text(raw)
    }
}

fn content_type_header(headers: &[Header]) -> String {
    headers
        .iter()
        .find(|header| header.enabled && header.key.eq_ignore_ascii_case("content-type"))
        .map(|header| header.value.to_ascii_lowercase())
        .unwrap_or_default()
}

fn serialize_graphql_body(graphql: Value) -> Option<String> {
    match graphql {
        Value::Null => None,
        Value::Object(mut payload) => {
            if let Some(Value::String(variables)) = payload.get_mut("variables") {
                let trimmed = variables.trim();
                let normalized = if trimmed.is_empty() {
                    Value::Object(serde_json::Map::new())
                } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    parsed
                } else {
                    Value::String(variables.clone())
                };
                payload.insert("variables".to_string(), normalized);
            }

            serde_json::to_string_pretty(&Value::Object(payload)).ok()
        }
        other => serde_json::to_string_pretty(&other).ok(),
    }
}

fn map_file_body(
    file: Option<PostmanFileBody>,
    headers: &[Header],
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> RequestBody {
    let Some(file) = file else {
        warnings.push(ImportWarning::new(
            Some(path_label(path)),
            "File request body metadata was missing, so the body was skipped.",
        ));
        return RequestBody::None;
    };

    if let Some(content) = file.content.filter(|content| !content.is_empty()) {
        return infer_text_body(content, headers, None, file.src.as_deref());
    }

    warnings.push(ImportWarning::new(
        Some(path_label(path)),
        match file.src.as_deref().filter(|src| !src.is_empty()) {
            Some(src) => format!(
                "File request body `{src}` could not be imported because Setu cannot retain Postman file references yet."
            ),
            None => "File request body had no inline content and was skipped.".to_string(),
        },
    ));
    RequestBody::None
}

fn url_has_query_key(url: &str, key: &str) -> bool {
    let query = url
        .split_once('#')
        .map(|(prefix, _)| prefix)
        .unwrap_or(url)
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default();

    query.split('&').any(|pair| {
        let current_key = pair.split_once('=').map(|(key, _)| key).unwrap_or(pair);
        current_key == key
            || urlencoding::decode(current_key)
                .map(|decoded| decoded == key)
                .unwrap_or(false)
    })
}

fn append_query_param(url: &mut String, key: &str, value: &str) {
    let (prefix, fragment) = url
        .split_once('#')
        .map(|(prefix, fragment)| (prefix, Some(fragment)))
        .unwrap_or((url.as_str(), None));
    let separator = if prefix.contains('?') {
        if prefix.ends_with('?') || prefix.ends_with('&') {
            ""
        } else {
            "&"
        }
    } else {
        "?"
    };

    let mut next_url = String::from(prefix);
    next_url.push_str(separator);
    next_url.push_str(key);
    if !value.is_empty() {
        next_url.push('=');
        next_url.push_str(value);
    }

    if let Some(fragment) = fragment {
        next_url.push('#');
        next_url.push_str(fragment);
    }

    *url = next_url;
}

fn warn_on_unsupported_fields(
    warnings: &mut Vec<ImportWarning>,
    path: &[String],
    condition: bool,
    message: impl Into<String>,
) {
    if condition {
        warnings.push(ImportWarning::new(Some(path_label(path)), message));
    }
}

fn path_label(path: &[String]) -> String {
    path.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_nested_postman_tree_and_supported_request_fields() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Team API",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "item": [
            {
              "name": "Users",
              "item": [
                {
                  "name": "Create User",
                  "request": {
                    "method": "POST",
                    "header": [
                      { "key": "Content-Type", "value": "application/json" }
                    ],
                    "body": {
                      "mode": "raw",
                      "raw": "{\"name\":\"raunak\"}",
                      "options": { "raw": { "language": "json" } }
                    },
                    "url": {
                      "raw": "https://api.example.com/users"
                    }
                  }
                },
                {
                  "name": "Search Users",
                  "request": {
                    "method": "GET",
                    "url": {
                      "protocol": "https",
                      "host": ["api", "example", "com"],
                      "path": ["users", "search"],
                      "query": [
                        { "key": "q", "value": "admin" }
                      ]
                    }
                  }
                }
              ]
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("team-api.postman_collection.json"), json)
            .expect("import succeeds");

        assert_eq!(result.provider, "Postman");
        assert_eq!(result.collection.name, "Team API");
        assert_eq!(result.collection.folder_count(), 1);
        assert_eq!(result.collection.request_count(), 2);
        assert!(result.warnings.is_empty());

        let users = match &result.collection.nodes[0] {
            ImportedNode::Folder { name, children } => {
                assert_eq!(name, "Users");
                children
            }
            ImportedNode::Request { .. } => panic!("expected folder"),
        };

        let request = match &users[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.url, "https://api.example.com/users");
        match &request.body {
            RequestBody::Json(body) => assert!(body.contains("raunak")),
            other => panic!("expected json body, got {other:?}"),
        }
    }

    #[test]
    fn imports_string_requests_and_inherited_supported_auth() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Auth Imports",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "auth": {
            "type": "bearer",
            "bearer": [{ "key": "token", "value": "collection-token" }]
          },
          "item": [
            {
              "name": "Health",
              "request": "https://api.example.com/health"
            },
            {
              "name": "Users",
              "auth": {
                "type": "apikey",
                "apikey": [
                  { "key": "key", "value": "api_key" },
                  { "key": "value", "value": "folder-secret" },
                  { "key": "in", "value": "query" }
                ]
              },
              "item": [
                {
                  "name": "List Users",
                  "request": {
                    "method": "GET",
                    "url": "https://api.example.com/users"
                  }
                },
                {
                  "name": "Create User",
                  "request": {
                    "method": "POST",
                    "auth": {
                      "type": "basic",
                      "basic": [
                        { "key": "username", "value": "raunak" },
                        { "key": "password", "value": "secret" }
                      ]
                    },
                    "url": "https://api.example.com/users"
                  }
                }
              ]
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("auth-imports.json"), json)
            .expect("import succeeds");

        assert_eq!(result.collection.request_count(), 3);
        assert!(result.warnings.is_empty());

        let health = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert_eq!(health.method, HttpMethod::Get);
        assert_eq!(health.url, "https://api.example.com/health");
        assert!(health.headers.iter().any(|header| {
            header.key == "Authorization" && header.value == "Bearer collection-token"
        }));

        let users = match &result.collection.nodes[1] {
            ImportedNode::Folder { children, .. } => children,
            ImportedNode::Request { .. } => panic!("expected folder"),
        };

        let list_users = match &users[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert_eq!(
            list_users.url,
            "https://api.example.com/users?api_key=folder-secret"
        );

        let create_user = match &users[1] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert!(create_user.headers.iter().any(|header| {
            header.key == "Authorization" && header.value == "Basic cmF1bmFrOnNlY3JldA=="
        }));
    }

    #[test]
    fn imports_graphql_file_bodies_and_raw_headers() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Bodies",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "item": [
            {
              "name": "GraphQL Search",
              "request": {
                "method": "POST",
                "header": "Content-Type: application/json\nX-Trace: abc123",
                "body": {
                  "mode": "graphql",
                  "graphql": {
                    "query": "query Search($limit: Int!) { search(limit: $limit) { id } }",
                    "variables": "{\"limit\":10}"
                  }
                },
                "url": "https://api.example.com/graphql"
              }
            },
            {
              "name": "Upload Manifest",
              "request": {
                "method": "POST",
                "header": [
                  { "key": "Content-Type", "value": "application/json" }
                ],
                "body": {
                  "mode": "file",
                  "file": {
                    "src": "manifest.json",
                    "content": "{\"ok\":true}"
                  }
                },
                "url": "https://api.example.com/upload"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("bodies.json"), json)
            .expect("import succeeds");

        assert!(result.warnings.is_empty());

        let graphql_request = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert!(graphql_request
            .headers
            .iter()
            .any(|header| header.key == "X-Trace" && header.value == "abc123"));
        match &graphql_request.body {
            RequestBody::Json(body) => {
                let value: Value = serde_json::from_str(body).expect("graphql body is json");
                assert_eq!(
                    value["query"],
                    "query Search($limit: Int!) { search(limit: $limit) { id } }"
                );
                assert_eq!(value["variables"]["limit"], 10);
            }
            other => panic!("expected json body, got {other:?}"),
        }

        let file_request = match &result.collection.nodes[1] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        match &file_request.body {
            RequestBody::Json(body) => assert_eq!(body, "{\"ok\":true}"),
            other => panic!("expected json body, got {other:?}"),
        }
    }

    #[test]
    fn respects_noauth_and_does_not_override_explicit_auth_headers() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Auth Precedence",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "auth": {
            "type": "bearer",
            "bearer": [{ "key": "token", "value": "collection-token" }]
          },
          "item": [
            {
              "name": "Public",
              "request": {
                "auth": { "type": "noauth" },
                "url": "https://api.example.com/public"
              }
            },
            {
              "name": "Pinned Header",
              "request": {
                "method": "GET",
                "header": [
                  { "key": "Authorization", "value": "Bearer explicit-token" }
                ],
                "auth": {
                  "type": "basic",
                  "basic": [
                    { "key": "username", "value": "raunak" },
                    { "key": "password", "value": "secret" }
                  ]
                },
                "url": "https://api.example.com/private"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("auth-precedence.json"), json)
            .expect("import succeeds");

        assert_eq!(result.collection.request_count(), 2);

        let public = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert!(!public
            .headers
            .iter()
            .any(|header| header.key.eq_ignore_ascii_case("authorization")));

        let pinned = match &result.collection.nodes[1] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        let auth_headers: Vec<_> = pinned
            .headers
            .iter()
            .filter(|header| header.key.eq_ignore_ascii_case("authorization"))
            .collect();
        assert_eq!(auth_headers.len(), 1);
        assert_eq!(auth_headers[0].value, "Bearer explicit-token");
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("Auth header `Authorization` was not added")));
    }

    #[test]
    fn appends_query_api_keys_without_duplication_and_keeps_fragments() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Query API Keys",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "item": [
            {
              "name": "Append Key",
              "request": {
                "method": "GET",
                "auth": {
                  "type": "apikey",
                  "apikey": [
                    { "key": "key", "value": "api_key" },
                    { "key": "value", "value": "secret" },
                    { "key": "in", "value": "query" }
                  ]
                },
                "url": "https://api.example.com/users?page=1#details"
              }
            },
            {
              "name": "Skip Duplicate",
              "request": {
                "method": "GET",
                "auth": {
                  "type": "apikey",
                  "apikey": [
                    { "key": "key", "value": "api_key" },
                    { "key": "value", "value": "secret" },
                    { "key": "in", "value": "query" }
                  ]
                },
                "url": "https://api.example.com/users?api_key=existing"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("query-api-keys.json"), json)
            .expect("import succeeds");

        let append_key = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert_eq!(
            append_key.url,
            "https://api.example.com/users?page=1&api_key=secret#details"
        );

        let skip_duplicate = match &result.collection.nodes[1] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert_eq!(
            skip_duplicate.url,
            "https://api.example.com/users?api_key=existing"
        );
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("API key query parameter `api_key` was not added")));
    }

    #[test]
    fn warns_for_invalid_raw_headers_and_non_inline_file_bodies() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Importer Warnings",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "item": [
            {
              "name": "Upload From Path",
              "request": {
                "method": "POST",
                "header": "Content-Type: text/plain\nBroken Header",
                "body": {
                  "mode": "file",
                  "file": {
                    "src": "payload.txt"
                  }
                },
                "url": "https://api.example.com/upload"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("importer-warnings.json"), json)
            .expect("import succeeds");

        let request = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert!(request
            .headers
            .iter()
            .any(|header| header.key == "Content-Type" && header.value == "text/plain"));
        assert!(matches!(request.body, RequestBody::None));
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("raw header line could not be parsed")));
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("cannot retain Postman file references")));
    }

    #[test]
    fn skips_disabled_bodies() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Disabled Bodies",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "item": [
            {
              "name": "Disabled Raw Body",
              "request": {
                "method": "POST",
                "body": {
                  "mode": "raw",
                  "raw": "{\"ignored\":true}",
                  "disabled": true,
                  "options": { "raw": { "language": "json" } }
                },
                "url": "https://api.example.com/submit"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("disabled-bodies.json"), json)
            .expect("import succeeds");

        let request = match &result.collection.nodes[0] {
            ImportedNode::Request { request } => request,
            ImportedNode::Folder { .. } => panic!("expected request"),
        };
        assert!(matches!(request.body, RequestBody::None));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn warns_for_unsupported_fields_and_duplicate_urlencoded_keys() {
        let importer = PostmanCollectionImporter;
        let json = r#"
        {
          "info": {
            "name": "Warnings",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
          },
          "event": [{ "listen": "prerequest" }],
          "item": [
            {
              "name": "Submit",
              "event": [{ "listen": "test" }],
              "request": {
                "method": "POST",
                "auth": { "type": "digest" },
                "body": {
                  "mode": "urlencoded",
                  "urlencoded": [
                    { "key": "role", "value": "admin" },
                    { "key": "role", "value": "owner" }
                  ]
                },
                "url": "https://api.example.com/submit"
              }
            }
          ]
        }"#;

        let result = importer
            .import(Path::new("warnings.json"), json)
            .expect("import succeeds");

        assert_eq!(result.collection.request_count(), 1);
        assert!(result.warnings.len() >= 3);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("Collection scripts")));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("auth type `digest`")));
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("Duplicate x-www-form-urlencoded key")));
    }
}
