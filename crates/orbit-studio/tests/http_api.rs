//! End-to-end HTTP API test against the real loopback server.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

use orbit_studio::{Server, ServerConfig};

fn request(port: u16, raw: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, head.to_string(), body.to_string())
}

fn get(port: u16, path: &str, token: &str) -> (u16, String, String) {
    request(
        port,
        &format!(
            "GET {path}{}token={token} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            if path.contains('?') { "&" } else { "?" }
        ),
    )
}

#[test]
fn loopback_api_serves_studio_and_runs_a_mission() {
    let server = Server::bind(&ServerConfig {
        port: 0,
        open_browser: false,
        token: Some("test-token-0123456789".into()),
    })
    .unwrap();
    let port = server.local_addr().unwrap().port();
    let token = server.token().to_string();
    thread::spawn(move || server.run());

    let (status, head, body) = request(
        port,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(status, 200);
    assert!(head.contains("Content-Security-Policy"));
    assert!(body.contains("ORBIT"));

    let (status, _, body) = get(port, "/api/health", "wrong-token");
    assert_eq!(status, 401);
    assert!(body.contains("token"));

    let (status, _, body) = get(port, "/api/health", &token);
    assert_eq!(status, 200);
    assert!(body.contains("\"ok\":true"));

    let (status, _, body) = get(port, "/api/conservation", &token);
    assert_eq!(status, 200);
    assert!(body.contains("relative_energy_error"));

    // Validate a preset rocket.
    let rocket = orbit_studio::model::rocket_to_json(&orbit_core::rocket::presets::atlas());
    let body_text = orbit_studio::json::stringify(&rocket);
    let (status, _, body) = request(
        port,
        &format!(
            "POST /api/validate?token={token} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body_text.len(),
            body_text
        ),
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"ok\":true"));

    // Start a real mission.
    let payload = r#"{"script_id":"lunar-atlas-100","rocket_id":"atlas"}"#;
    let (status, _, body) = request(
        port,
        &format!(
            "POST /api/mission/run?token={token} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        ),
    );
    assert_eq!(status, 200);
    let run_id = body
        .split("\"run_id\":")
        .nth(1)
        .and_then(|s| s.trim_end_matches('}').parse::<u64>().ok())
        .expect("run id");

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut complete = false;
    while Instant::now() < deadline {
        let (_, _, body) = get(port, &format!("/api/runs/{run_id}"), &token);
        if body.contains("\"status\":\"complete\"") {
            assert!(body.contains("\"success\":true"), "{body}");
            complete = true;
            break;
        }
        if body.contains("\"status\":\"failed\"") {
            panic!("mission failed: {body}");
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(complete, "mission did not complete");

    let (status, _, body) = get(port, &format!("/api/export/csv?run={run_id}"), &token);
    assert_eq!(status, 200);
    assert!(body.starts_with("time_s,phase,altitude_m"));
    assert!(body.lines().count() > 100);

    let (status, _, body) = get(port, &format!("/api/export/report?run={run_id}"), &token);
    assert_eq!(status, 200);
    assert!(body.contains("ORBIT MISSION REPORT"));

    let (status, _, body) = get(port, &format!("/api/events?run={run_id}"), &token);
    assert_eq!(status, 200);
    assert!(body.contains("event: mission"));
    assert!(body.contains("event: done"));
}
