use anyhow::{anyhow, Result};
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

        warn_on_unsupported_fields(
            &mut warnings,
            &collection_path,
            document.auth.is_some(),
            "Collection auth is not imported yet.",
        );
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
            .filter_map(|item| import_item(item, &collection_path, &mut warnings))
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
    auth: Option<Value>,
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
    auth: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanRequestValue {
    Request(PostmanRequest),
    UnsupportedString(String),
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
    auth: Option<Value>,
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
    file: Option<Value>,
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

fn import_item(
    item: PostmanItem,
    parent_path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Option<ImportedNode> {
    let label = item
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Untitled".to_string());
    let mut item_path = parent_path.to_vec();
    item_path.push(label.clone());

    warn_on_unsupported_fields(
        warnings,
        &item_path,
        item.auth.is_some(),
        "Item auth is not imported yet.",
    );
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
            .filter_map(|child| import_item(child, &item_path, warnings))
            .collect();

        return Some(ImportedNode::Folder {
            name: label,
            children,
        });
    }

    let request = match item.request {
        Some(PostmanRequestValue::Request(request)) => request,
        Some(PostmanRequestValue::UnsupportedString(raw_request)) => {
            warnings.push(ImportWarning::new(
                Some(path_label(&item_path)),
                format!(
                    "String-based Postman requests are not supported by the importer{}.",
                    if raw_request.is_empty() { "" } else { " yet" }
                ),
            ));
            return None;
        }
        None => {
            warnings.push(ImportWarning::new(
                Some(path_label(&item_path)),
                "This Postman item did not contain a request and was skipped.",
            ));
            return None;
        }
    };

    import_request(label, request, &item_path, warnings)
        .map(|request| ImportedNode::Request { request })
}

fn import_request(
    name: String,
    request: PostmanRequest,
    path: &[String],
    warnings: &mut Vec<ImportWarning>,
) -> Option<RequestData> {
    warn_on_unsupported_fields(
        warnings,
        path,
        request.auth.is_some(),
        "Request auth is not imported yet.",
    );

    let method = map_method(request.method.as_deref(), path, warnings)?;
    let url = match request.url {
        Some(url) => map_url(url),
        None => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                "Request URL is missing, so the request was skipped.",
            ));
            return None;
        }
    };

    let headers = map_headers(request.header, path, warnings);
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
                format!("HTTP method `{unsupported}` is not supported in Setu yet. The request was skipped."),
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
        Some(PostmanHeaders::Raw(raw_headers)) => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                format!(
                    "Raw Postman header blocks are skipped ({} characters).",
                    raw_headers.len()
                ),
            ));
            Vec::new()
        }
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

    match body.mode.as_deref().unwrap_or("raw") {
        "raw" => {
            let raw = body.raw.unwrap_or_default();
            let language = body
                .options
                .and_then(|options| options.raw)
                .and_then(|raw| raw.language)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let content_type = headers
                .iter()
                .find(|header| header.key.eq_ignore_ascii_case("content-type"))
                .map(|header| header.value.to_ascii_lowercase())
                .unwrap_or_default();

            if language == "json" || content_type.contains("application/json") {
                RequestBody::Json(raw)
            } else {
                RequestBody::Text(raw)
            }
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
                                format!("Form-data file field `{key}` has no file source and was skipped."),
                            ));
                        }
                    },
                    _ => fields.push(MultipartField::text(key, entry.value.unwrap_or_default())),
                }
            }
            RequestBody::MultipartFormData(fields)
        }
        "graphql" => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                "GraphQL request bodies are not supported yet. The body was imported as raw text when possible.",
            ));
            body.raw
                .map(RequestBody::Text)
                .or_else(|| {
                    body.graphql
                        .map(|value| RequestBody::Text(value.to_string()))
                })
                .unwrap_or(RequestBody::None)
        }
        "file" => {
            let has_file_payload = body.file.is_some();
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                if has_file_payload {
                    "File request bodies are not supported yet and were skipped."
                } else {
                    "File request bodies are not supported yet."
                },
            ));
            RequestBody::None
        }
        other => {
            warnings.push(ImportWarning::new(
                Some(path_label(path)),
                format!("Postman body mode `{other}` is not supported yet. The body was skipped."),
            ));
            RequestBody::None
        }
    }
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
                "auth": { "type": "bearer" },
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
            .any(|warning| warning.message.contains("Request auth")));
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("Duplicate x-www-form-urlencoded key")));
    }
}
