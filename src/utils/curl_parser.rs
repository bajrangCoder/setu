use std::collections::HashMap;

use crate::entities::{Header, HttpMethod, RequestBody};

/// Result of parsing a curl command.
#[derive(Debug, Clone)]
pub struct ParsedCurl {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<Header>,
    pub body: RequestBody,
}

/// Quick check whether a string looks like a curl command.
pub fn looks_like_curl(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("curl ") || trimmed.starts_with("curl\t") || trimmed.starts_with("curl\n")
}

/// Parse a `curl` command string into a [`ParsedCurl`].
///
/// Supports the most common flags:
/// `-X/--request`, `-H/--header`, `-d/--data/--data-raw/--data-binary/--data-urlencode`,
/// `-F/--form`, `-u/--user`, `-A/--user-agent`, `-e/--referer`, `-b/--cookie`,
/// `--url`, `--location`, `--get`, `-G`, `--compressed`, `--insecure`, `-k`.
pub fn parse_curl(input: &str) -> Result<ParsedCurl, String> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err("Empty curl command".into());
    }

    let mut iter = tokens.into_iter().peekable();

    // Skip leading "curl"
    let first = iter.next().ok_or("Missing curl")?;
    if !first.eq_ignore_ascii_case("curl") {
        return Err("Command does not start with curl".into());
    }

    let mut url: Option<String> = None;
    let mut method: Option<HttpMethod> = None;
    let mut headers: Vec<Header> = Vec::new();
    let mut data_parts: Vec<String> = Vec::new();
    let mut form_parts: Vec<(String, String)> = Vec::new();
    let mut basic_auth: Option<String> = None;
    let mut force_get = false;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-X" | "--request" => {
                let v = iter.next().ok_or("Missing value for -X")?;
                method = Some(parse_method(&v));
            }
            "-H" | "--header" => {
                let v = iter.next().ok_or("Missing value for -H")?;
                if let Some(h) = parse_header(&v) {
                    headers.push(h);
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                let v = iter.next().ok_or("Missing value for -d")?;
                data_parts.push(v);
            }
            "--data-urlencode" => {
                let v = iter.next().ok_or("Missing value for --data-urlencode")?;
                data_parts.push(v);
            }
            "-F" | "--form" | "--form-string" => {
                let v = iter.next().ok_or("Missing value for -F")?;
                if let Some((k, val)) = split_once(&v, '=') {
                    form_parts.push((k.to_string(), val.to_string()));
                }
            }
            "-u" | "--user" => {
                let v = iter.next().ok_or("Missing value for -u")?;
                basic_auth = Some(v);
            }
            "-A" | "--user-agent" => {
                let v = iter.next().ok_or("Missing value for -A")?;
                headers.push(Header::new("User-Agent", v));
            }
            "-e" | "--referer" => {
                let v = iter.next().ok_or("Missing value for -e")?;
                headers.push(Header::new("Referer", v));
            }
            "-b" | "--cookie" => {
                let v = iter.next().ok_or("Missing value for -b")?;
                headers.push(Header::new("Cookie", v));
            }
            "--url" => {
                let v = iter.next().ok_or("Missing value for --url")?;
                url = Some(v);
            }
            "-G" | "--get" => {
                force_get = true;
            }
            // Flags we silently ignore (no value).
            "--location" | "-L" | "--compressed" | "--insecure" | "-k" | "--silent" | "-s"
            | "--verbose" | "-v" | "--fail" | "-f" | "--http1.1" | "--http2" | "--http2-prior-knowledge"
            | "--no-buffer" | "-N" | "--include" | "-i" | "--head" | "-I" | "--globoff" | "-g" => {}
            // Flags we ignore but consume their value.
            "-o" | "--output" | "--max-time" | "--connect-timeout" | "--retry"
            | "--retry-delay" | "--retry-max-time" | "-w" | "--write-out" | "--cacert"
            | "--cert" | "--key" | "-T" | "--upload-file" | "--resolve" | "--proxy" | "-x"
            | "--proxy-user" | "--limit-rate" | "--range" | "-r" | "--ciphers" => {
                let _ = iter.next();
            }
            _ => {
                if arg.starts_with("--") {
                    // Unknown long flag: consume optional value if next isn't a flag.
                    if matches!(iter.peek(), Some(next) if !next.starts_with('-')) {
                        // Heuristic: leave alone — many long flags are boolean.
                    }
                } else if arg.starts_with('-') && arg.len() > 1 {
                    // Unknown short flag, ignore.
                } else {
                    // Positional: treat as URL (first one wins).
                    if url.is_none() {
                        url = Some(arg);
                    }
                }
            }
        }
    }

    let url = url.ok_or("Could not find URL in curl command")?;

    if let Some(creds) = basic_auth {
        let encoded = base64_encode(creds.as_bytes());
        headers.push(Header::new("Authorization", format!("Basic {}", encoded)));
    }

    let body = build_body(&data_parts, &form_parts, &headers);

    let method = if force_get {
        HttpMethod::Get
    } else {
        method.unwrap_or_else(|| {
            if !matches!(body, RequestBody::None) {
                HttpMethod::Post
            } else {
                HttpMethod::Get
            }
        })
    };

    Ok(ParsedCurl {
        method,
        url,
        headers,
        body,
    })
}

fn parse_method(value: &str) -> HttpMethod {
    match value.to_ascii_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        "PATCH" => HttpMethod::Patch,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

fn parse_header(raw: &str) -> Option<Header> {
    let (key, value) = split_once(raw, ':')?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() {
        None
    } else {
        Some(Header::new(key, value))
    }
}

fn split_once(s: &str, sep: char) -> Option<(&str, &str)> {
    let idx = s.find(sep)?;
    Some((&s[..idx], &s[idx + sep.len_utf8()..]))
}

fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn build_body(
    data_parts: &[String],
    form_parts: &[(String, String)],
    headers: &[Header],
) -> RequestBody {
    if !form_parts.is_empty() {
        let fields = form_parts
            .iter()
            .map(|(k, v)| crate::entities::MultipartField::text(k, v))
            .collect::<Vec<_>>();
        return RequestBody::MultipartFormData(fields);
    }

    if data_parts.is_empty() {
        return RequestBody::None;
    }

    let combined = data_parts.join("&");
    let content_type = header_value(headers, "Content-Type").unwrap_or("");

    if content_type.contains("application/json") || looks_like_json(&combined) {
        return RequestBody::Json(combined);
    }

    if content_type.contains("application/x-www-form-urlencoded")
        || (combined.contains('=') && !combined.contains('\n'))
    {
        let mut map = HashMap::new();
        for pair in combined.split('&') {
            if let Some((k, v)) = split_once(pair, '=') {
                map.insert(k.to_string(), v.to_string());
            } else if !pair.is_empty() {
                map.insert(pair.to_string(), String::new());
            }
        }
        if !map.is_empty() {
            return RequestBody::FormData(map);
        }
    }

    RequestBody::Text(combined)
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

/// Tokenize a shell-style command line, honoring single quotes, double quotes,
/// backslash escapes, and line continuations (`\` at end of line).
fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut chars = input.chars().peekable();

    enum Quote {
        None,
        Single,
        Double,
    }
    let mut quote = Quote::None;

    while let Some(c) = chars.next() {
        match quote {
            Quote::None => match c {
                '\\' => {
                    if let Some(&next) = chars.peek() {
                        // Line continuation: consume newline (and following CR).
                        if next == '\n' {
                            chars.next();
                            continue;
                        }
                        if next == '\r' {
                            chars.next();
                            if let Some(&'\n') = chars.peek() {
                                chars.next();
                            }
                            continue;
                        }
                        chars.next();
                        current.push(next);
                        in_token = true;
                    }
                }
                '\'' => {
                    quote = Quote::Single;
                    in_token = true;
                }
                '"' => {
                    quote = Quote::Double;
                    in_token = true;
                }
                c if c.is_whitespace() => {
                    if in_token {
                        tokens.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                }
                _ => {
                    current.push(c);
                    in_token = true;
                }
            },
            Quote::Single => match c {
                '\'' => quote = Quote::None,
                _ => current.push(c),
            },
            Quote::Double => match c {
                '"' => quote = Quote::None,
                '\\' => {
                    if let Some(&next) = chars.peek() {
                        match next {
                            '"' | '\\' | '`' | '$' | '\n' => {
                                chars.next();
                                if next != '\n' {
                                    current.push(next);
                                }
                            }
                            _ => current.push('\\'),
                        }
                    }
                }
                _ => current.push(c),
            },
        }
    }

    if !matches!(quote, Quote::None) {
        return Err("Unterminated quote in curl command".into());
    }
    if in_token {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Minimal RFC 4648 base64 encoder (no external dependency).
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | input[i + 2] as u32;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = (input[i] as u32) << 16;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_curl_prefix() {
        assert!(looks_like_curl("curl https://example.com"));
        assert!(looks_like_curl("  curl https://example.com"));
        assert!(!looks_like_curl("https://example.com"));
        assert!(!looks_like_curl("curlhttps://example.com"));
    }

    #[test]
    fn parses_simple_get() {
        let parsed = parse_curl("curl https://api.example.com/users").unwrap();
        assert_eq!(parsed.method, HttpMethod::Get);
        assert_eq!(parsed.url, "https://api.example.com/users");
        assert!(parsed.headers.is_empty());
        assert!(matches!(parsed.body, RequestBody::None));
    }

    #[test]
    fn parses_explicit_method() {
        let parsed = parse_curl("curl -X DELETE https://api.example.com/users/1").unwrap();
        assert_eq!(parsed.method, HttpMethod::Delete);
        assert_eq!(parsed.url, "https://api.example.com/users/1");
    }

    #[test]
    fn parses_headers_and_json_body() {
        let cmd = r#"curl -X POST 'https://api.example.com/users' \
            -H 'Content-Type: application/json' \
            -H 'Accept: application/json' \
            -d '{"name":"alice"}'"#;
        let parsed = parse_curl(cmd).unwrap();
        assert_eq!(parsed.method, HttpMethod::Post);
        assert_eq!(parsed.url, "https://api.example.com/users");
        assert_eq!(parsed.headers.len(), 2);
        match &parsed.body {
            RequestBody::Json(s) => assert_eq!(s, r#"{"name":"alice"}"#),
            other => panic!("expected JSON body, got {:?}", other),
        }
    }

    #[test]
    fn defaults_to_post_when_data_present_without_method() {
        let parsed = parse_curl("curl https://api.example.com/x -d 'hello=world'").unwrap();
        assert_eq!(parsed.method, HttpMethod::Post);
    }

    #[test]
    fn parses_form_urlencoded() {
        let parsed = parse_curl(
            "curl -X POST https://api.example.com -d 'a=1' -d 'b=2' \
             -H 'Content-Type: application/x-www-form-urlencoded'",
        )
        .unwrap();
        match &parsed.body {
            RequestBody::FormData(map) => {
                assert_eq!(map.get("a").map(String::as_str), Some("1"));
                assert_eq!(map.get("b").map(String::as_str), Some("2"));
            }
            other => panic!("expected FormData body, got {:?}", other),
        }
    }

    #[test]
    fn parses_multipart_form() {
        let parsed = parse_curl(
            "curl https://api.example.com -F 'name=alice' -F 'email=alice@example.com'",
        )
        .unwrap();
        match &parsed.body {
            RequestBody::MultipartFormData(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].key, "name");
                assert_eq!(fields[0].value, "alice");
            }
            other => panic!("expected MultipartFormData body, got {:?}", other),
        }
    }

    #[test]
    fn parses_basic_auth() {
        let parsed = parse_curl("curl -u alice:secret https://api.example.com").unwrap();
        let auth = parsed
            .headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("Authorization"))
            .expect("auth header");
        assert!(auth.value.starts_with("Basic "));
    }

    #[test]
    fn handles_location_flag() {
        let parsed =
            parse_curl("curl --location 'https://ssp.veonadx.com/bid/prebid'").unwrap();
        assert_eq!(parsed.url, "https://ssp.veonadx.com/bid/prebid");
    }

    #[test]
    fn parses_real_world_multiline_curl() {
        let cmd = r#"curl --location 'https://ssp.veonadx.com/bid/prebid' \
--header 'Content-Type: application/json' \
--data '{
    "id": "abc",
    "test": 1
}'"#;
        let parsed = parse_curl(cmd).unwrap();
        assert_eq!(parsed.method, HttpMethod::Post);
        assert_eq!(parsed.url, "https://ssp.veonadx.com/bid/prebid");
        match parsed.body {
            RequestBody::Json(s) => {
                assert!(s.contains("\"id\""));
                assert!(s.contains("\"test\""));
            }
            other => panic!("expected JSON body, got {:?}", other),
        }
    }

    #[test]
    fn parses_user_provided_complex_curl() {
        let cmd = r#"curl --location 'https://ssp.veonadx.com/bid/prebid' \
--header 'Content-Type: application/json' \
--data '{
    "imp": [
        {
            "id": "18e78361",
            "ext": {
                "prebid": {
                    "is_rewarded_inventory": 1
                }
            }
        }
    ],
    "id": "18e78361",
    "regs": {
        "ext": {
            "gdpr": 1
        }
    }
}'"#;
        let parsed = parse_curl(cmd).unwrap();
        assert_eq!(parsed.method, HttpMethod::Post);
        assert_eq!(parsed.url, "https://ssp.veonadx.com/bid/prebid");
        assert_eq!(parsed.headers.len(), 1);
        assert_eq!(parsed.headers[0].key, "Content-Type");
        assert_eq!(parsed.headers[0].value, "application/json");
        match parsed.body {
            RequestBody::Json(s) => {
                assert!(s.contains("\"imp\""));
                assert!(s.contains("\"gdpr\""));
            }
            other => panic!("expected JSON body, got {:?}", other),
        }
    }
}
