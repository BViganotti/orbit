//! Orbit launcher. Starts the loopback Studio and opens the browser.

use std::io::Write;

use orbit_studio::{Server, ServerConfig};

fn main() {
    let mut config = ServerConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-open" => config.open_browser = false,
            "--port" => {
                let Some(value) = args.next() else {
                    eprintln!("orbit: --port requires a value");
                    std::process::exit(2);
                };
                match value.parse::<u16>() {
                    Ok(port) => config.port = port,
                    Err(_) => {
                        eprintln!("orbit: invalid port '{value}'");
                        std::process::exit(2);
                    }
                }
            }
            "--token" => {
                let Some(value) = args.next() else {
                    eprintln!("orbit: --token requires a value");
                    std::process::exit(2);
                };
                if value.len() < 16 || value.len() > 128 {
                    eprintln!("orbit: token must be 16..=128 bytes");
                    std::process::exit(2);
                }
                config.token = Some(value);
            }
            "--help" | "-h" => {
                println!("Usage: orbit [--port N] [--no-open] [--token TOKEN]");
                return;
            }
            other => {
                eprintln!("orbit: unknown argument '{other}'");
                std::process::exit(2);
            }
        }
    }
    let server = match Server::bind(&config) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("orbit: could not bind 127.0.0.1: {}", error);
            std::process::exit(1);
        }
    };
    let url = server.url();
    println!("ORBIT_URL={url}");
    let _ = std::io::stdout().flush();
    server.open_browser();
    server.run();
}
