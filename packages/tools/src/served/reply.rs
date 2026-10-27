//! One request of a served page answered: its head read within a deadline, the file it names
//! found inside its directory, as the workspace allows, and sent with its type.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use super::Root;

/// How long a request may take to arrive: a client that sends nothing holds no other.
const READ: Duration = Duration::from_secs(2);
/// The most a request's head may be.
const HEAD: usize = 16 * 1024;

/// One request read and answered, then the connection closed.
pub(super) fn answer(mut stream: TcpStream, roots: &Mutex<HashMap<String, Root>>) {
    let _ = stream.set_read_timeout(Some(READ));
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    while !head.windows(4).any(|end| end == b"\r\n\r\n") && head.len() < HEAD {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => head.extend_from_slice(chunk.get(..read).unwrap_or_default()),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let mut words = head.lines().next().unwrap_or_default().split(' ');
    let (method, target) = (
        words.next().unwrap_or_default(),
        words.next().unwrap_or_default(),
    );
    let (status, kind, body) = match method {
        "GET" | "HEAD" => found(target, referer(&head), roots),
        _ => ("405 Method Not Allowed", "text/plain", b"only GET".to_vec()),
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes());
    if method == "GET" {
        let _ = stream.write_all(&body);
    }
    let _ = stream.shutdown(Shutdown::Both);
}

/// The `Referer` of a request's head: the address of the page that asked.
fn referer(head: &str) -> Option<&str> {
    head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("referer").then(|| value.trim())
    })
}

/// The first segment of the path of `address`, an absolute address or a path.
fn first_segment(address: &str) -> &str {
    let path = address.split_once("://").map_or(address, |(_, rest)| {
        rest.find('/').map_or("", |at| rest.get(at..).unwrap_or(""))
    });
    (path.trim_start_matches('/'))
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
}

/// What `target` names: its status, its type and its bytes. A path with no prefix of a served
/// root (a built site names `/assets/app.css`) is read in the root its `referer` is of, else in
/// the only root served.
fn found(
    target: &str,
    referer: Option<&str>,
    roots: &Mutex<HashMap<String, Root>>,
) -> (&'static str, &'static str, Vec<u8>) {
    let missing = ("404 Not Found", "text/plain", b"not found".to_vec());
    let refused = ("403 Forbidden", "text/plain", b"not served".to_vec());
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let mut parts = path.trim_start_matches('/').split('/').peekable();
    let roots = roots.lock().unwrap_or_else(PoisonError::into_inner);
    let root = match parts.peek().and_then(|prefix| roots.get(*prefix)) {
        Some(root) => {
            parts.next();
            Some(root.clone())
        }
        None => referer
            .and_then(|from| roots.get(first_segment(from)))
            .or_else(|| match roots.len() {
                1 => roots.values().next(),
                _ => None,
            })
            .cloned(),
    };
    drop(roots);
    let Some(root) = root else {
        return missing;
    };
    let mut file = root.dir.clone();
    for part in parts.filter(|part| !part.is_empty()) {
        let Some(name) = decoded(part) else {
            return refused;
        };
        if name.starts_with('.') || name.contains(['/', '\\']) {
            return refused;
        }
        file.push(name);
    }
    if file.is_dir() {
        file.push("index.html");
    }
    let readable = file.to_str().map(|path| root.workspace.readable(path));
    let inside = std::fs::canonicalize(&file).is_ok_and(|real| real.starts_with(&root.dir));
    match (readable, inside) {
        (Some(Ok(_)), true) => match std::fs::read(&file) {
            Ok(bytes) => ("200 OK", kind(&file), bytes),
            Err(_) => missing,
        },
        (Some(Ok(_)), false) if !file.exists() => missing,
        (Some(Ok(_) | Err(_)) | None, _) => refused,
    }
}

/// A file's type, by its extension, as a browser reads it.
fn kind(file: &Path) -> &'static str {
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// A path's segment with its `%XX` escapes read; `None` when they are not UTF-8.
fn decoded(part: &str) -> Option<String> {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let hex = part.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}
