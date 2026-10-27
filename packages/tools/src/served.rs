//! A page on disk opened in the window as a site: its directory served on the loopback, so the
//! model gives a path (`demo/shop/index.html`) and the page loads as from a server, its styles,
//! scripts, `fetch` and storage working, with no server of anyone's to start. Each directory has
//! an unguessable prefix of its own; only its files are served, never one whose name starts with
//! a dot (`.env`, `.git`), outside it once links are followed, or off limits to the tools.

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use contracts::tool::ToolError;

use crate::workspace::Workspace;

mod reply;

use reply::answer;

/// The most connections answered at once; one more waits for the oldest to end, within the
/// time a request may take to arrive.
const AT_ONCE: usize = 16;

/// The directories opened as sites in one window, served while it lives.
#[derive(Default)]
pub struct Served(Mutex<Option<Running>>);

/// A directory served: where it is, and the workspace whose limits it keeps.
#[derive(Clone)]
struct Root {
    dir: PathBuf,
    workspace: Workspace,
}

struct Running {
    port: u16,
    roots: Arc<Mutex<HashMap<String, Root>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// What a `url` the model gave is: a file for certain (a path that starts as one, a page's
/// extension, `file://`), maybe a file (a bare name that may be a site's), or an address.
enum Given<'a> {
    File(&'a str),
    Maybe(&'a str),
    Address,
}

fn given(url: &str) -> Given<'_> {
    let url = url.trim();
    if let Some(path) = url.strip_prefix("file://") {
        return Given::File(path);
    }
    if url.contains("://") || url.starts_with("about:") {
        return Given::Address;
    }
    let lower = url.to_ascii_lowercase();
    let page = [".html", ".htm", ".xhtml", ".svg"]
        .iter()
        .any(|end| lower.ends_with(end));
    match url.starts_with(['/', '.', '~']) || page {
        true => Given::File(url),
        false => Given::Maybe(url),
    }
}

impl Served {
    /// The address the page `url` names is served at, read with `workspace`'s limits; `None`
    /// when `url` is a web address, for the browser to read.
    pub fn address(&self, workspace: &Workspace, url: &str) -> Result<Option<String>, ToolError> {
        let (path, sure) = match given(url) {
            Given::File(path) => (path, true),
            Given::Maybe(path) => (path, false),
            Given::Address => return Ok(None),
        };
        let wanted = workspace.readable(path)?;
        let page = match wanted.is_dir() {
            true => wanted.join("index.html"),
            false => wanted,
        };
        if !page.is_file() {
            return match sure {
                true => Err(ToolError::Failed(format!(
                    "there is no file {}: give a page's path, or a web address",
                    workspace.shown(&page)
                ))),
                false => Ok(None),
            };
        }
        let page = std::fs::canonicalize(&page)
            .map_err(|error| ToolError::Failed(format!("{}: {error}", page.display())))?;
        // A page elsewhere (a download, a system file) would hand its whole directory to the
        // scripts it holds.
        if !page.starts_with(workspace.root()) {
            return Err(ToolError::Failed(format!(
                "{} is not inside the workspace {}: only its pages open from disk",
                page.display(),
                workspace.root().display()
            )));
        }
        let (Some(dir), Some(name)) = (page.parent(), page.file_name()) else {
            return Err(ToolError::Failed(format!(
                "{} is not a page",
                page.display()
            )));
        };
        let mut running = crate::lock(&self.0);
        if running.is_none() {
            *running = Some(start()?);
        }
        let Some(running) = running.as_ref() else {
            return Err(ToolError::Failed("the pages' server did not start".into()));
        };
        let prefix = running.prefix(dir, workspace);
        let name = encoded(&name.to_string_lossy());
        Ok(Some(format!(
            "http://127.0.0.1:{}/{prefix}/{name}",
            running.port
        )))
    }
}

impl Running {
    /// The prefix `dir` is served under: its own, made the first time it is opened.
    fn prefix(&self, dir: &Path, workspace: &Workspace) -> String {
        let mut roots = crate::lock(&self.roots);
        if let Some((prefix, _)) = roots.iter().find(|(_, root)| root.dir == dir) {
            return prefix.clone();
        }
        let prefix = format!("llx-{}", token());
        let root = Root {
            dir: dir.to_path_buf(),
            workspace: workspace.clone(),
        };
        roots.insert(prefix.clone(), root);
        prefix
    }
}

fn start() -> Result<Running, ToolError> {
    let failed =
        |error: std::io::Error| ToolError::Failed(format!("pages cannot be served: {error}"));
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(failed)?;
    let port = listener.local_addr().map_err(failed)?.port();
    let roots: Arc<Mutex<HashMap<String, Root>>> = Arc::default();
    let stop = Arc::new(AtomicBool::new(false));
    let (served, stopped) = (Arc::clone(&roots), Arc::clone(&stop));
    let thread = std::thread::Builder::new()
        .name("llx-served".into())
        .spawn(move || serve(&listener, &served, &stopped))
        .map_err(failed)?;
    Ok(Running {
        port,
        roots,
        stop,
        thread: Some(thread),
    })
}

/// Each connection answered on a thread of its own: a browser opens some ahead of the requests
/// it sends on them, and one still silent must hold up no other. Each is joined, at the latest
/// when the server stops.
fn serve(listener: &TcpListener, roots: &Arc<Mutex<HashMap<String, Root>>>, stopped: &AtomicBool) {
    let mut answering: Vec<JoinHandle<()>> = Vec::new();
    for stream in listener.incoming() {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let (done, going): (Vec<_>, Vec<_>) =
            answering.into_iter().partition(JoinHandle::is_finished);
        answering = going;
        done.into_iter().for_each(|thread| drop(thread.join()));
        if answering.len() >= AT_ONCE {
            drop(answering.remove(0).join());
        }
        let roots = Arc::clone(roots);
        let thread = std::thread::Builder::new()
            .name("llx-served-answer".into())
            .spawn(move || answer(stream, &roots));
        if let Ok(thread) = thread {
            answering.push(thread);
        }
    }
    answering.into_iter().for_each(|thread| drop(thread.join()));
}

impl Drop for Running {
    /// The server stops with the window's state: told, woken by a connection of its own, joined.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// `name` as one segment of an address: what is not plain spelled `%XX`.
fn encoded(name: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// 32 hex characters from the clock and the address space; unguessable to a page in one run.
pub(crate) fn token() -> String {
    use std::collections::hash_map::RandomState;
    use std::fmt::Write as _;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        let _ = write!(out, "{:016x}", hasher.finish());
    }
    out
}

#[cfg(test)]
#[path = "served_tests.rs"]
mod tests;
