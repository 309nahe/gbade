//! # gbade-lsp
//!
//! A deliberately small Language Server Protocol client, just enough for
//! GBADE's editor to use **clangd** (bundled with the toolchain):
//!
//! - live errors and warnings while typing (`textDocument/publishDiagnostics`),
//! - completion (`textDocument/completion`),
//! - hover documentation (`textDocument/hover`),
//! - go to definition (`textDocument/definition`).
//!
//! The protocol is JSON-RPC over clangd's stdin/stdout: each message is
//! `Content-Length: N\r\n\r\n` followed by N bytes of JSON. A reader thread
//! parses incoming messages into a channel; the UI calls [`LspClient::poll`]
//! once per frame, so nothing ever blocks the interface.
//!
//! clangd learns how files are compiled from `build/compile_commands.json`,
//! written by `gbade_toolchain::builder::write_compile_commands` from the very
//! same flags the build uses.
//!
//! Positions use the "utf-32" encoding when clangd accepts it: a column is
//! then a number of characters, exactly like the editor's cursor.

use gbade_toolchain::{Diagnostic, Severity, diagnostics::hint_for};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

/// A completion proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// What the list shows, e.g. `key_hit(u16 keys)`.
    pub label: String,
    /// What gets inserted, e.g. `key_hit`.
    pub insert: String,
    /// Extra information, e.g. the return type `bool`.
    pub detail: String,
    /// A short kind name: "function", "macro", "variable", "type"...
    pub kind: &'static str,
}

/// A position in a file: 0-based line and character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    pub line: u32,
    pub character: u32,
}

/// Something the server sent back.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The complete current list of problems in a file (replaces the old one).
    Diagnostics(PathBuf, Vec<Diagnostic>),
    /// Answer to [`LspClient::completion`].
    Completion { id: i64, items: Vec<CompletionItem> },
    /// Answer to [`LspClient::hover`]: plain text, empty if nothing to say.
    Hover { id: i64, text: String },
    /// Answer to [`LspClient::definition`].
    Definition { id: i64, target: Option<(PathBuf, Pos)> },
    /// The server stopped (crashed or exited).
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Initialize,
    Completion,
    Hover,
    Definition,
    Other,
}

/// A running language server.
pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Option<Value>>,
    next_id: i64,
    pending: HashMap<i64, Pending>,
    ready: bool,
    /// Messages to send once the `initialize` handshake is done.
    queue: Vec<Value>,
    versions: HashMap<PathBuf, i32>,
    utf32: bool,
}

impl LspClient {
    /// Start clangd for a project. `compile_commands_dir` is the folder
    /// containing `compile_commands.json` (the project's `build/`).
    pub fn start(clangd: &Path, root: &Path, compile_commands_dir: &Path) -> std::io::Result<LspClient> {
        // GBADE_CLANGD_LOG=/some/file saves clangd's own detailed log, to
        // troubleshoot the language server.
        let log = std::env::var_os("GBADE_CLANGD_LOG").and_then(|p| std::fs::File::create(p).ok());
        let mut child = Command::new(clangd)
            .arg(format!("--compile-commands-dir={}", compile_commands_dir.display()))
            // Index in the background: fine for small GBA projects, and it
            // makes "go to definition" work across files.
            .arg("--background-index")
            .arg("--header-insertion=never")
            .arg(if log.is_some() { "--log=verbose" } else { "--log=error" })
            .arg("--clang-tidy=false")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log.map(Stdio::from).unwrap_or_else(Stdio::null))
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("clangd-reader".into())
            .spawn(move || reader_thread(stdout, tx))?;
        let mut client = LspClient {
            child,
            stdin,
            rx,
            next_id: 1,
            pending: HashMap::new(),
            ready: false,
            queue: Vec::new(),
            versions: HashMap::new(),
            utf32: false,
        };
        let root_uri = path_to_uri(root);
        client.request_now(
            Pending::Initialize,
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "general": { "positionEncodings": ["utf-32", "utf-16"] },
                    "textDocument": {
                        "publishDiagnostics": { "relatedInformation": false },
                        "completion": { "completionItem": { "snippetSupport": false } },
                        "hover": { "contentFormat": ["plaintext"] },
                        "synchronization": { "didSave": false }
                    }
                },
                "initializationOptions": { "clangdFileStatus": false }
            }),
        )?;
        Ok(client)
    }

    fn write(&mut self, msg: &Value) -> std::io::Result<()> {
        let body = serde_json::to_string(msg).expect("valid JSON");
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
        self.stdin.flush()
    }

    fn request_now(&mut self, kind: Pending, method: &str, params: Value) -> std::io::Result<i64> {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(id)
    }

    /// Send now if initialised, else queue until the handshake completes.
    fn send(&mut self, msg: Value) {
        if self.ready {
            let _ = self.write(&msg);
        } else {
            self.queue.push(msg);
        }
    }

    fn request(&mut self, kind: Pending, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, kind);
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Tell the server a file is open in the editor, with its current text.
    pub fn did_open(&mut self, path: &Path, text: &str) {
        self.versions.insert(path.to_path_buf(), 1);
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": path_to_uri(path), "languageId": language_id(path), "version": 1, "text": text}}),
        );
    }

    /// Send the new full text of an open file.
    pub fn did_change(&mut self, path: &Path, text: &str) {
        let v = self.versions.entry(path.to_path_buf()).or_insert(1);
        *v += 1;
        let version = *v;
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": path_to_uri(path), "version": version}, "contentChanges": [{"text": text}]}),
        );
    }

    pub fn did_close(&mut self, path: &Path) {
        self.versions.remove(path);
        self.notify("textDocument/didClose", json!({"textDocument": {"uri": path_to_uri(path)}}));
    }

    pub fn is_open(&self, path: &Path) -> bool {
        self.versions.contains_key(path)
    }

    fn position(&self, path: &Path, pos: Pos) -> Value {
        json!({"textDocument": {"uri": path_to_uri(path)}, "position": {"line": pos.line, "character": pos.character}})
    }

    /// Ask for completions at `pos`. The answer arrives as [`Event::Completion`].
    pub fn completion(&mut self, path: &Path, pos: Pos) -> i64 {
        let params = self.position(path, pos);
        self.request(Pending::Completion, "textDocument/completion", params)
    }

    /// Ask what the symbol at `pos` is. The answer arrives as [`Event::Hover`].
    pub fn hover(&mut self, path: &Path, pos: Pos) -> i64 {
        let params = self.position(path, pos);
        self.request(Pending::Hover, "textDocument/hover", params)
    }

    /// Ask where the symbol at `pos` is defined ([`Event::Definition`]).
    pub fn definition(&mut self, path: &Path, pos: Pos) -> i64 {
        let params = self.position(path, pos);
        self.request(Pending::Definition, "textDocument/definition", params)
    }

    /// True when columns are counted in characters (utf-32). With utf-16 they
    /// are the same for every character the editor can type in practice.
    pub fn uses_utf32(&self) -> bool {
        self.utf32
    }

    /// Process everything the server sent since the last call.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            let Some(msg) = msg else {
                events.push(Event::Exited);
                break;
            };
            self.handle(msg, &mut events);
        }
        events
    }

    /// Wait (up to `timeout`) until `pred` matches an event, collecting all
    /// events. Used by tests and the CLI; the UI uses [`LspClient::poll`].
    pub fn wait_for(&mut self, timeout: std::time::Duration, pred: impl Fn(&Event) -> bool) -> Vec<Event> {
        let deadline = std::time::Instant::now() + timeout;
        let mut all = Vec::new();
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match self.rx.recv_timeout(remaining.min(std::time::Duration::from_millis(50))) {
                Ok(Some(msg)) => {
                    let mut ev = Vec::new();
                    self.handle(msg, &mut ev);
                    let done = ev.iter().any(&pred);
                    all.extend(ev);
                    if done {
                        break;
                    }
                }
                Ok(None) => {
                    all.push(Event::Exited);
                    break;
                }
                Err(_) => {}
            }
        }
        all
    }

    fn handle(&mut self, msg: Value, events: &mut Vec<Event>) {
        // A request from the server (e.g. "window/workDoneProgress/create"):
        // answer with an empty result so it does not wait for us.
        if let (Some(id), Some(_)) = (msg.get("id"), msg.get("method")) {
            let _ = self.write(&json!({"jsonrpc": "2.0", "id": id, "result": null}));
            return;
        }
        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            if method == "textDocument/publishDiagnostics" {
                if let Some((path, diags)) = parse_diagnostics(&msg["params"]) {
                    events.push(Event::Diagnostics(path, diags));
                }
            }
            return;
        }
        let Some(id) = msg.get("id").and_then(Value::as_i64) else { return };
        let Some(kind) = self.pending.remove(&id) else { return };
        let result = &msg["result"];
        match kind {
            Pending::Initialize => {
                self.utf32 = result["capabilities"]["positionEncoding"].as_str() == Some("utf-32");
                let _ = self.write(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
                self.ready = true;
                for m in std::mem::take(&mut self.queue) {
                    let _ = self.write(&m);
                }
            }
            Pending::Completion => events.push(Event::Completion { id, items: parse_completion(result) }),
            Pending::Hover => events.push(Event::Hover { id, text: parse_hover(result) }),
            Pending::Definition => events.push(Event::Definition { id, target: parse_location(result) }),
            Pending::Other => {}
        }
    }

    /// Politely stop the server (also done on drop).
    pub fn shutdown(&mut self) {
        if self.ready {
            let _ = self.request_now(Pending::Other, "shutdown", Value::Null);
            let _ = self.write(&json!({"jsonrpc": "2.0", "method": "exit"}));
        }
        // Give it a moment, then make sure it is gone.
        for _ in 0..20 {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn reader_thread(stdout: impl Read, tx: Sender<Option<Value>>) {
    let mut r = BufReader::new(stdout);
    loop {
        let mut len: Option<usize> = None;
        loop {
            let mut line = String::new();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
                Ok(_) => {}
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = v.trim().parse().ok();
            }
        }
        let Some(len) = len else { continue };
        let mut body = vec![0u8; len];
        if r.read_exact(&mut body).is_err() {
            let _ = tx.send(None);
            return;
        }
        if let Ok(v) = serde_json::from_slice::<Value>(&body) {
            if tx.send(Some(v)).is_err() {
                return;
            }
        }
    }
}

fn language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("cpp" | "cc" | "hpp") => "cpp",
        _ => "c",
    }
}

/// `/a b/c.c` -> `file:///a%20b/c.c`
pub fn path_to_uri(path: &Path) -> String {
    let mut s = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => s.push(b as char),
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// The inverse of [`path_to_uri`].
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// clangd capitalises messages ("Use of undeclared identifier"); the hint
/// table and the build's own messages use clang's lowercase style.
fn decapitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) if f.is_uppercase() && c.clone().next().is_some_and(|n| n.is_lowercase()) => {
            f.to_lowercase().collect::<String>() + c.as_str()
        }
        _ => s.to_string(),
    }
}

/// Convert `publishDiagnostics` params into GBADE diagnostics (1-based lines).
pub fn parse_diagnostics(params: &Value) -> Option<(PathBuf, Vec<Diagnostic>)> {
    let path = uri_to_path(params["uri"].as_str()?)?;
    let mut out = Vec::new();
    for d in params["diagnostics"].as_array()? {
        let severity = match d["severity"].as_u64() {
            Some(1) => Severity::Error,
            Some(2) => Severity::Warning,
            _ => Severity::Note,
        };
        let message = decapitalize(d["message"].as_str().unwrap_or("").lines().next().unwrap_or(""));
        let start = &d["range"]["start"];
        out.push(Diagnostic {
            severity,
            file: Some(path.clone()),
            line: start["line"].as_u64().unwrap_or(0) as u32 + 1,
            column: start["character"].as_u64().unwrap_or(0) as u32 + 1,
            hint: hint_for(&message),
            message,
        });
    }
    Some((path, out))
}

fn completion_kind(k: u64) -> &'static str {
    match k {
        2 | 3 | 4 => "function",
        5 | 10 => "field",
        6 => "variable",
        7 | 22 => "type",
        13 => "enum",
        20 => "enum value",
        14 => "keyword",
        15 => "snippet",
        17 => "file",
        21 => "constant",
        25 => "type parameter",
        _ => "",
    }
}

pub fn parse_completion(result: &Value) -> Vec<CompletionItem> {
    let items = match result {
        Value::Array(a) => a.as_slice(),
        Value::Object(_) => result["items"].as_array().map(Vec::as_slice).unwrap_or(&[]),
        _ => &[],
    };
    items
        .iter()
        .map(|it| {
            let label = it["label"].as_str().unwrap_or("").trim().to_string();
            let insert = it["textEdit"]["newText"]
                .as_str()
                .or_else(|| it["insertText"].as_str())
                .or_else(|| it["filterText"].as_str())
                .unwrap_or(&label)
                .to_string();
            CompletionItem {
                label,
                insert,
                detail: it["detail"].as_str().unwrap_or("").to_string(),
                kind: completion_kind(it["kind"].as_u64().unwrap_or(0)),
            }
        })
        .filter(|c| !c.insert.is_empty())
        .collect()
}

pub fn parse_hover(result: &Value) -> String {
    let c = &result["contents"];
    let text = match c {
        Value::String(s) => s.clone(),
        Value::Object(_) => c["value"].as_str().unwrap_or("").to_string(),
        Value::Array(a) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v["value"].as_str().unwrap_or("").to_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    text.trim().to_string()
}

pub fn parse_location(result: &Value) -> Option<(PathBuf, Pos)> {
    let loc = match result {
        Value::Array(a) => a.first()?,
        Value::Object(_) => result,
        _ => return None,
    };
    let uri = loc["uri"].as_str().or_else(|| loc["targetUri"].as_str())?;
    let range = if loc.get("range").is_some() { &loc["range"] } else { &loc["targetSelectionRange"] };
    Some((
        uri_to_path(uri)?,
        Pos {
            line: range["start"]["line"].as_u64().unwrap_or(0) as u32,
            character: range["start"]["character"].as_u64().unwrap_or(0) as u32,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_roundtrip() {
        let p = Path::new("/Users/me/GBADE Projects/my-game/src/main.c");
        let u = path_to_uri(p);
        assert_eq!(u, "file:///Users/me/GBADE%20Projects/my-game/src/main.c");
        assert_eq!(uri_to_path(&u).unwrap(), p);
        let accents = Path::new("/home/ana/jeux/éte.c");
        assert_eq!(uri_to_path(&path_to_uri(accents)).unwrap(), accents);
        assert!(uri_to_path("http://x").is_none());
    }

    #[test]
    fn diagnostics_are_converted_with_hints() {
        let params = json!({
            "uri": "file:///p/src/main.c",
            "diagnostics": [
                {"range": {"start": {"line": 4, "character": 2}, "end": {"line": 4, "character": 3}},
                 "severity": 1, "message": "Use of undeclared identifier 'y'"},
                {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
                 "severity": 2, "message": "Unused variable 'x'\n\nmore"}
            ]
        });
        let (path, d) = parse_diagnostics(&params).unwrap();
        assert_eq!(path, PathBuf::from("/p/src/main.c"));
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].line, d[0].column), (5, 3));
        assert_eq!(d[0].message, "use of undeclared identifier 'y'");
        assert!(d[0].hint.is_some(), "the beginner hint table applies to live errors too");
        assert_eq!(d[1].severity, Severity::Warning);
        assert_eq!(d[1].message, "unused variable 'x'");
    }

    #[test]
    fn completion_hover_and_location_parsing() {
        let c = parse_completion(&json!({"isIncomplete": false, "items": [
            {"label": " key_hit(u16 keys)", "kind": 3, "detail": "bool", "textEdit": {"newText": "key_hit", "range": {}}},
            {"label": "KEY_A", "kind": 21, "insertText": "KEY_A"}
        ]}));
        assert_eq!(c[0].label, "key_hit(u16 keys)");
        assert_eq!(c[0].insert, "key_hit");
        assert_eq!(c[0].kind, "function");
        assert_eq!(c[1].kind, "constant");
        assert_eq!(parse_hover(&json!({"contents": {"kind": "plaintext", "value": " function vsync\n"}})), "function vsync");
        assert_eq!(parse_hover(&Value::Null), "");
        let (p, pos) = parse_location(&json!([{"uri": "file:///sdk/gba.h", "range": {"start": {"line": 9, "character": 4}}}])).unwrap();
        assert_eq!(p, PathBuf::from("/sdk/gba.h"));
        assert_eq!(pos, Pos { line: 9, character: 4 });
        assert!(parse_location(&json!([])).is_none());
    }
}
