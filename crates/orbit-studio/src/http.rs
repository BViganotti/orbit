//! Minimal HTTP/1.1 request parsing and response writing for the local Studio.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::json::{self, Value};

pub const MAX_HEADER: usize = 32 * 1024;
pub const MAX_BODY: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Head,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Result<Value, String> {
        let text = std::str::from_utf8(&self.body).map_err(|_| "body is not UTF-8".to_string())?;
        json::parse(text).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Response {
    pub fn new(status: u16, content_type: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body: body.into(),
            headers: Vec::new(),
        }
    }

    pub fn json(status: u16, value: &Value) -> Self {
        let mut response = Self::new(
            status,
            "application/json; charset=utf-8",
            json::stringify(value).into_bytes(),
        );
        response
            .headers
            .push(("Cache-Control".into(), "no-store".into()));
        response
    }

    pub fn text(status: u16, text: impl Into<String>) -> Self {
        Self::new(status, "text/plain; charset=utf-8", text.into())
    }

    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(status, &json::object(vec![("error", json::string(message.into()))]))
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Read and parse one HTTP request from a connection.
pub fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end;
    loop {
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed before headers".into());
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_double_crlf(&buffer) {
            head_end = pos + 4;
            break;
        }
        if buffer.len() > MAX_HEADER {
            return Err("headers too large".into());
        }
    }
    let head = &buffer[..head_end];
    let request = parse_head(head)?;
    if request.header("transfer-encoding").is_some() {
        return Err("transfer-encoding is not supported".into());
    }
    let lengths: Vec<&str> = request
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.as_str())
        .collect();
    if lengths.len() > 1 {
        return Err("duplicate content-length".into());
    }
    let content_length = lengths
        .first()
        .map(|v| v.parse::<usize>().map_err(|_| "bad content-length".to_string()))
        .transpose()?
        .unwrap_or(0);
    if content_length > MAX_BODY {
        return Err("body too large".into());
    }
    let mut body = buffer[head_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_BODY {
            return Err("body too large".into());
        }
    }
    body.truncate(content_length);
    let mut request = request;
    request.body = body;
    Ok(request)
}

fn find_double_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn parse_head(head: &[u8]) -> Result<Request, String> {
    let text = std::str::from_utf8(head).map_err(|_| "headers are not UTF-8".to_string())?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split(' ');
    let method = match parts.next() {
        Some("GET") => Method::Get,
        Some("POST") => Method::Post,
        Some("HEAD") => Method::Head,
        Some(_) => return Err("unsupported method".into()),
        None => return Err("missing method".into()),
    };
    let target = parts.next().ok_or_else(|| "missing request target".to_string())?;
    let version = parts.next().ok_or_else(|| "missing HTTP version".to_string())?;
    if parts.next().is_some() || !version.starts_with("HTTP/1.") {
        return Err("malformed request line".into());
    }
    if target.len() > 4096 {
        return Err("request target too long".into());
    }
    let (raw_path, query) = target.split_once('?').unwrap_or((target, ""));
    if !raw_path.starts_with('/') {
        return Err("request target must be absolute path".into());
    }
    let path = percent_decode(raw_path)?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "malformed header".to_string())?;
        if name.len() > 64 || value.len() > 4096 || headers.len() >= 64 {
            return Err("header limit exceeded".into());
        }
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let mut query_pairs = Vec::new();
    if !query.is_empty() {
        if query.len() > 4096 {
            return Err("query too long".into());
        }
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            if query_pairs.len() >= 64 {
                return Err("too many query parameters".into());
            }
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let k = percent_decode(k)?;
            let v = percent_decode(v)?;
            if k.len() > 256 || v.len() > 2048 {
                return Err("query parameter too long".into());
            }
            query_pairs.push((k, v));
        }
    }
    Ok(Request {
        method,
        path,
        query: query_pairs,
        headers,
        body: Vec::new(),
    })
}

fn percent_decode(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("truncated percent escape".into());
                }
                let hi = hex_value(bytes[i + 1]).ok_or_else(|| "bad percent escape".to_string())?;
                let lo = hex_value(bytes[i + 2]).ok_or_else(|| "bad percent escape".to_string())?;
                out.push(hi * 16 + lo);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "invalid UTF-8 in percent escape".into())
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Write a complete response and close the connection.
pub fn write_response(stream: &mut TcpStream, response: &Response, head_only: bool) -> std::io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let mut head = String::with_capacity(256);
    head.push_str(&format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        status_text(response.status)
    ));
    head.push_str(&format!("Content-Type: {}\r\n", response.content_type));
    head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    head.push_str("Connection: close\r\n");
    head.push_str("X-Content-Type-Options: nosniff\r\n");
    head.push_str("X-Frame-Options: DENY\r\n");
    head.push_str("Referrer-Policy: no-referrer\r\n");
    head.push_str("Permissions-Policy: camera=(), microphone=(), geolocation=()\r\n");
    if response.content_type.starts_with("text/html") {
        head.push_str("Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'\r\n");
    }
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only {
        stream.write_all(&response.body)?;
    }
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_with_query() {
        let req = parse_head(
            b"GET /api/health?token=abc&x=1 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/api/health");
        assert_eq!(req.query_param("token"), Some("abc"));
        assert_eq!(req.header("host"), Some("localhost"));
    }

    #[test]
    fn parses_post_target() {
        let req = parse_head(b"POST /api/run HTTP/1.0\r\nContent-Type: application/json\r\n\r\n").unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.path, "/api/run");
        assert!(req.body.is_empty());
    }

    #[test]
    fn rejects_bad_requests() {
        for bad in [
            b"".as_slice(),
            b"GET".as_slice(),
            b"GET / HTTP/2.0\r\n\r\n".as_slice(),
            b"PUT / HTTP/1.1\r\n\r\n".as_slice(),
            b"GET relative HTTP/1.1\r\n\r\n".as_slice(),
            b"GET / HTTP/1.1\r\nBadHeader\r\n\r\n".as_slice(),
            b"GET /%zz HTTP/1.1\r\n\r\n".as_slice(),
        ] {
            assert!(parse_head(bad).is_err());
        }
    }

    #[test]
    fn percent_decoding_handles_utf8() {
        let req = parse_head("GET /x?name=caf%C3%A9 HTTP/1.1\r\n\r\n".as_bytes()).unwrap();
        assert_eq!(req.query_param("name"), Some("café"));
    }

    #[test]
    fn response_headers_are_hardened() {
        let r = Response::new(200, "text/html; charset=utf-8", b"<html></html>".to_vec());
        assert_eq!(r.status, 200);
        // Header emission is tested end-to-end in tests/http_api.rs; here we
        // assert the response is constructed with the expected type.
        assert!(r.content_type.starts_with("text/html"));
    }

    #[test]
    fn response_status_lines() {
        let r = Response::json(200, &json::object(vec![("ok", json::Value::Bool(true))]));
        assert_eq!(r.status, 200);
        assert!(r.content_type.starts_with("application/json"));
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;

    #[test]
    fn excessive_query_parameters_are_rejected() {
        let query = (0..65).map(|i| format!("k{i}=v")).collect::<Vec<_>>().join("&");
        let head = format!("GET /api/health?{query} HTTP/1.1\r\n\r\n");
        assert!(parse_head(head.as_bytes()).is_err());
    }

    #[test]
    fn oversized_targets_and_headers_are_rejected() {
        let long_path = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(5000));
        assert!(parse_head(long_path.as_bytes()).is_err());
        let header = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(5000));
        assert!(parse_head(header.as_bytes()).is_err());
    }
}
