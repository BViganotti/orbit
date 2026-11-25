//! Embedded UI assets. Everything is compiled into the binary.

pub const INDEX_HTML: &str = include_str!("../../../assets/index.html");
pub const APP_CSS: &str = include_str!("../../../assets/app.css");
pub const APP_JS: &str = include_str!("../../../assets/app.js");

pub fn content_type(path: &str) -> &'static str {
    match path {
        "/" | "/index.html" => "text/html; charset=utf-8",
        "/app.css" => "text/css; charset=utf-8",
        "/app.js" => "application/javascript; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_are_embedded_and_original() {
        assert!(INDEX_HTML.contains("ORBIT"));
        assert!(APP_CSS.contains("--accent"));
        assert!(APP_JS.contains("runMission"));
        assert!(!INDEX_HTML.contains("http://"));
        assert!(!INDEX_HTML.contains("https://"));
        assert!(!APP_CSS.contains("http://"));
        assert!(!APP_JS.contains("http://"));
    }
}
