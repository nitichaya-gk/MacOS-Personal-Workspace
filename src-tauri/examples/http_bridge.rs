//! Development only: exposes the same `rpc` dispatcher over HTTP on 127.0.0.1:1421 so the UI can be
//! exercised in a normal browser (`VITE_HTTP_BRIDGE=1 npm run dev`). Never shipped in the app.
//! `cargo run --example http_bridge -- /path/to/workspace`
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::commands::dispatch;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

fn main() {
    let root = std::env::args().nth(1).expect("usage: http_bridge <workspace folder>");
    let mut ws = Workspace::open(std::path::Path::new(&root)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:1421").unwrap();
    eprintln!("http bridge on 127.0.0.1:1421 for {root}");
    for stream in listener.incoming().flatten() {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            continue;
        }
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                break;
            }
            if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; len];
        let _ = reader.read_exact(&mut body);
        let path = request_line.split_whitespace().nth(1).unwrap_or("/").to_string();
        let (status, ctype, out): (&str, &str, Vec<u8>) = if let Some(id) = path.strip_prefix("/asset/") {
            match personal_workspace_lib::application::assets::read(&ws.root, id) {
                Ok((bytes, _)) => ("200 OK", "application/octet-stream", bytes),
                Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
            }
        } else {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let method = req["method"].as_str().unwrap_or("").to_string();
            match dispatch(&mut ws, &method, &req["params"]) {
                Ok(v) => ("200 OK", "application/json", serde_json::to_vec(&v).unwrap()),
                Err(e) => {
                    eprintln!("rpc {method} failed: {e}");
                    ("400 Bad Request", "application/json", serde_json::to_vec(&e).unwrap())
                }
            }
        };
        let mut s = stream;
        let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n", out.len());
        let _ = s.write_all(&out);
    }
}
