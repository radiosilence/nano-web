// Minimal HTTP/1.1 parser and response builder
// Zero dependencies, zero-copy where possible

use std::str;

#[derive(Debug)]
pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub _version: &'a str,
    pub headers: Vec<(&'a str, &'a str)>,
}

#[derive(Debug)]
pub enum ParseError {
    Incomplete,
    Invalid,
}

/// Parse HTTP request from buffer
/// Returns Ok(request, body_offset) or Err
pub fn parse_request(buf: &[u8]) -> Result<(HttpRequest<'_>, usize), ParseError> {
    let s = str::from_utf8(buf).map_err(|_| ParseError::Invalid)?;

    // Find end of headers
    let header_end = s.find("\r\n\r\n").ok_or(ParseError::Incomplete)?;
    let header_section = &s[..header_end];

    let mut lines = header_section.lines();

    // Parse request line
    let request_line = lines.next().ok_or(ParseError::Invalid)?;
    let mut parts = request_line.split_whitespace();

    let method = parts.next().ok_or(ParseError::Invalid)?;
    let path = parts.next().ok_or(ParseError::Invalid)?;
    let version = parts.next().ok_or(ParseError::Invalid)?;

    // Parse headers
    let mut headers = Vec::new();
    for line in lines {
        if let Some(colon_pos) = line.find(':') {
            let name = &line[..colon_pos];
            let value = line[colon_pos + 1..].trim();
            headers.push((name, value));
        }
    }

    Ok((
        HttpRequest {
            method,
            path,
            _version: version,
            headers,
        },
        header_end + 4, // +4 for "\r\n\r\n"
    ))
}

/// Build HTTP response
pub fn build_response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let mut response = format!("HTTP/1.1 {} {}\r\n", status, status_text).into_bytes();

    // Add headers
    for (name, value) in headers {
        response.extend_from_slice(name.as_bytes());
        response.extend_from_slice(b": ");
        response.extend_from_slice(value.as_bytes());
        response.extend_from_slice(b"\r\n");
    }

    // Add Content-Length
    response.extend_from_slice(b"Content-Length: ");
    response.extend_from_slice(body.len().to_string().as_bytes());
    response.extend_from_slice(b"\r\n");

    // End headers
    response.extend_from_slice(b"\r\n");

    // Add body
    response.extend_from_slice(body);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_request() {
        let req = b"GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (parsed, offset) = parse_request(req).unwrap();

        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.path, "/test");
        assert_eq!(parsed._version, "HTTP/1.1");
        assert_eq!(parsed.headers.len(), 1);
        assert_eq!(offset, req.len());
    }

    #[test]
    fn test_build_response() {
        let body = b"Hello, World!";
        let headers = [("Content-Type", "text/plain")];
        let response = build_response(200, &headers, body);

        let response_str = str::from_utf8(&response).unwrap();
        assert!(response_str.contains("HTTP/1.1 200 OK"));
        assert!(response_str.contains("Content-Type: text/plain"));
        assert!(response_str.contains("Content-Length: 13"));
        assert!(response_str.ends_with("Hello, World!"));
    }
}
