//! Loopback HTTP/SSE Studio server.

use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use orbit_core::rocket::presets;
use orbit_flight::diagnostics::{mission_report, trajectory_csv};
use orbit_flight::mission::scripts;

use crate::assets;
use crate::http::{read_request, write_response, Method, Request, Response};
use crate::json::{self, object, string, Value};
use crate::model;
use crate::state::{RunRecord, Store};
use crate::token::{random_token, tokens_equal};

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub open_browser: bool,
    pub token: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 0,
            open_browser: true,
            token: None,
        }
    }
}

/// A bound Studio server.
pub struct Server {
    listener: TcpListener,
    token: String,
    store: Arc<Store>,
    open_browser: bool,
    active: Arc<AtomicUsize>,
}

const MAX_CONNECTIONS: usize = 64;

struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Server {
    pub fn bind(config: &ServerConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", config.port))?;
        Ok(Self {
            listener,
            token: config.token.clone().unwrap_or_else(random_token),
            store: Arc::new(Store::new()),
            open_browser: config.open_browser,
            active: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn url(&self) -> String {
        let addr = self
            .listener
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)));
        format!("http://{}/?token={}", addr, self.token)
    }

    pub fn open_browser(&self) {
        if self.open_browser {
            crate::open::open_browser(&self.url());
        }
    }

    /// Serve until the process exits.
    pub fn run(self) -> ! {
        let stop = Arc::new(AtomicBool::new(false));
        self.run_until(stop)
    }

    /// Serve until `stop` is set. Each connection is handled on its own thread.
    pub fn run_until(self, stop: Arc<AtomicBool>) -> ! {
        loop {
            if stop.load(Ordering::Relaxed) {
                std::process::exit(0);
            }
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    let token = self.token.clone();
                    let store = self.store.clone();
                    let active = self.active.clone();
                    thread::spawn(move || {
                        if active.fetch_add(1, Ordering::Relaxed) >= MAX_CONNECTIONS {
                            active.fetch_sub(1, Ordering::Relaxed);
                            let mut stream = stream;
                            let _ = write_response(
                                &mut stream,
                                &Response::error(503, "too many concurrent connections"),
                                false,
                            );
                            return;
                        }
                        let _guard = ConnectionGuard(active);
                        let _ = handle_connection(stream, &token, &store);
                    });
                }
                Err(_) => {
                    if stop.load(Ordering::Relaxed) {
                        std::process::exit(0);
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

fn authorized(request: &Request, token: &str) -> bool {
    if let Some(candidate) = request.query_param("token") {
        return tokens_equal(candidate, token);
    }
    if let Some(candidate) = request.header("x-orbit-token") {
        return tokens_equal(candidate, token);
    }
    false
}

fn handle_connection(mut stream: TcpStream, token: &str, store: &Arc<Store>) -> std::io::Result<()> {
    let request = match read_request(&mut stream) {
        Ok(r) => r,
        Err(error) => {
            let _ = write_response(&mut stream, &Response::error(400, error), false);
            return Ok(());
        }
    };

    // Static assets are public; every API call is token protected.
    if request.path.starts_with("/api/") && !authorized(&request, token) {
        write_response(
            &mut stream,
            &Response::error(401, "missing or invalid Studio token"),
            request.method == Method::Head,
        )?;
        return Ok(());
    }

    if request.path == "/api/events" {
        return serve_events(&mut stream, &request, store);
    }

    let response = route(&request, store);
    write_response(&mut stream, &response, request.method == Method::Head)
}

fn route(request: &Request, store: &Arc<Store>) -> Response {
    match (request.method, request.path.as_str()) {
        (Method::Get, "/") | (Method::Get, "/index.html") => {
            Response::new(200, assets::content_type("/"), assets::INDEX_HTML)
        }
        (Method::Get, "/app.css") => {
            Response::new(200, assets::content_type("/app.css"), assets::APP_CSS)
        }
        (Method::Get, "/app.js") => {
            Response::new(200, assets::content_type("/app.js"), assets::APP_JS)
        }
        (Method::Get, "/favicon.ico") => Response::new(204, "image/x-icon", Vec::new()),
        (Method::Get, "/api/health") => Response::json(
            200,
            &object(vec![
                ("ok", Value::Bool(true)),
                ("version", string(env!("CARGO_PKG_VERSION"))),
                ("model", string("two-dimensional coplanar")),
            ]),
        ),
        (Method::Get, "/api/presets") => {
            let rockets = presets::all()
                .iter()
                .map(model::rocket_to_json)
                .collect();
            Response::json(200, &object(vec![("rockets", json::array(rockets))]))
        }
        (Method::Get, "/api/scripts") => {
            let items = scripts::SCRIPTS
                .iter()
                .map(|script| {
                    object(vec![
                        ("id", string(script.id)),
                        ("name", string(script.name)),
                        ("rocket_id", string(script.rocket_id)),
                        ("description", string(script.description)),
                        ("lunar_altitude_km", json::number(script.lunar_altitude_km)),
                        ("epoch_days", json::number(script.epoch_days)),
                        ("moon_phase_deg", json::number(script.moon_phase_deg)),
                        ("sun_phase_deg", json::number(script.sun_phase_deg)),
                    ])
                })
                .collect();
            Response::json(200, &object(vec![("scripts", json::array(items))]))
        }
        (Method::Get, "/api/conservation") => match model::conservation_json() {
            Ok(value) => Response::json(200, &value),
            Err(error) => Response::error(500, error),
        },
        (Method::Post, "/api/validate") => match request.json() {
            Ok(value) => match model::validate_rocket(&value) {
                Ok(value) => Response::json(200, &value),
                Err(error) => Response::error(400, error),
            },
            Err(error) => Response::error(400, error),
        },
        (Method::Post, "/api/mission/run") => match request.json() {
            Ok(value) => {
                let record = store.create();
                let id = record.id;
                let thread_record = record.clone();
                thread::spawn(move || execute_run(thread_record, value));
                Response::json(200, &object(vec![("run_id", json::number(id as f64))]))
            }
            Err(error) => Response::error(400, error),
        },
        (Method::Post, "/api/demo") => match request.json() {
            Ok(value) => match model::run_request(&value) {
                Ok((_rocket, _config, result)) => {
                    Response::json(200, &model::mission_to_json(&result))
                }
                Err(error) => Response::error(400, error),
            },
            Err(error) => Response::error(400, error),
        },
        (Method::Get, path) if path.starts_with("/api/runs/") => {
            let id = path.trim_start_matches("/api/runs/").parse::<u64>();
            match id.ok().and_then(|id| store.get(id)) {
                Some(run) => {
                    let status = run.status();
                    let mut entries = vec![
                        ("status", string(status.as_str())),
                        ("run_id", json::number(run.id as f64)),
                    ];
                    if let Some(result) = run.result() {
                        entries.push(("result", result));
                    }
                    if let crate::state::RunStatus::Failed(message) = status {
                        entries.push(("message", string(message)));
                    }
                    Response::json(200, &object(entries))
                }
                None => Response::error(404, "run not found"),
            }
        }
        (Method::Get, "/api/export/csv") => export(request, store, "csv"),
        (Method::Get, "/api/export/report") => export(request, store, "report"),
        _ => Response::error(404, "not found"),
    }
}

fn export(request: &Request, store: &Arc<Store>, kind: &str) -> Response {
    let id = request
        .query_param("run")
        .and_then(|v| v.parse::<u64>().ok());
    let run = match id.and_then(|id| store.get(id)) {
        Some(r) => r,
        None => return Response::error(404, "run not found"),
    };
    match kind {
        "csv" => match run.csv() {
            Some(csv) => Response::new(200, "text/csv; charset=utf-8", csv)
                .with_header("Content-Disposition", "attachment; filename=\"orbit-trajectory.csv\""),
            None => Response::error(409, "run has no CSV yet"),
        },
        _ => match run.report() {
            Some(report) => Response::new(200, "text/plain; charset=utf-8", report)
                .with_header("Content-Disposition", "attachment; filename=\"orbit-report.txt\""),
            None => Response::error(409, "run has no report yet"),
        },
    }
}

fn execute_run(record: Arc<RunRecord>, value: Value) {
    let progress = |message: &str| {
        record.emit(
            "progress",
            json::stringify(&object(vec![("message", string(message))])),
        );
    };
    progress("validating vehicle and mission");
    let result = match model::run_request(&value) {
        Ok((_rocket, _config, result)) => result,
        Err(error) => {
            record.fail(error);
            return;
        }
    };
    progress("propagating ascent, transfer and capture");
    let value = model::mission_to_json(&result);
    if let Some(events) = value.array("events") {
        for event in events {
            record.emit("mission", json::stringify(event));
        }
    }
    let csv = trajectory_csv(&result);
    let report = mission_report(&result);
    let summary = json::stringify(&object(vec![
        ("run_id", json::number(record.id as f64)),
        ("success", Value::Bool(result.success)),
        ("status", string(result.status.as_str())),
        ("message", string(result.message.clone())),
    ]));
    record.finish(value, csv, report, summary);
}

fn serve_events(stream: &mut TcpStream, request: &Request, store: &Arc<Store>) -> std::io::Result<()> {
    let id = match request.query_param("run").and_then(|v| v.parse::<u64>().ok()) {
        Some(id) => id,
        None => {
            return write_response(stream, &Response::error(400, "missing run id"), false);
        }
    };
    let run = match store.get(id) {
        Some(run) => run,
        None => return write_response(stream, &Response::error(404, "run not found"), false),
    };
    let (past, receiver) = match run.subscribe() {
        Some(v) => v,
        None => return write_response(stream, &Response::error(429, "too many subscribers"), false),
    };
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n",
    )?;
    stream.flush()?;
    let mut terminal = false;
    for event in past {
        write_sse(stream, &event.name, &event.data)?;
        if event.name == "done" || event.name == "error" {
            terminal = true;
        }
    }
    if !terminal && !run.is_terminal() {
        loop {
            match receiver.recv_timeout(Duration::from_secs(12)) {
                Ok(event) => {
                    write_sse(stream, &event.name, &event.data)?;
                    if event.name == "done" || event.name == "error" {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    stream.write_all(b": ping\n\n")?;
                    stream.flush()?;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
    Ok(())
}

fn write_sse(stream: &mut TcpStream, name: &str, data: &str) -> std::io::Result<()> {
    stream.write_all(format!("event: {name}\n").as_bytes())?;
    for line in data.lines() {
        stream.write_all(b"data: ")?;
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_check_is_exact() {
        let request = crate::http::parse_head(b"GET /api/health?token=abc HTTP/1.1\r\n\r\n").unwrap();
        assert!(authorized(&request, "abc"));
        assert!(!authorized(&request, "abd"));
        let request = crate::http::parse_head(b"GET /api/health HTTP/1.1\r\nX-Orbit-Token: abc\r\n\r\n").unwrap();
        assert!(authorized(&request, "abc"));
    }

    #[test]
    fn static_routes_are_embedded() {
        let store = Arc::new(Store::new());
        let request = crate::http::parse_head(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let response = route(&request, &store);
        assert_eq!(response.status, 200);
        assert!(String::from_utf8_lossy(&response.body).contains("ORBIT"));
    }

    #[test]
    fn api_requires_token_before_routing() {
        let request = crate::http::parse_head(b"GET /api/health HTTP/1.1\r\n\r\n").unwrap();
        assert!(!authorized(&request, "secret"));
    }
}
