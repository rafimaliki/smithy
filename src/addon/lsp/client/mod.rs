//! One language server process: spawned on first use, spoken to over its stdio
//! from a reader thread, and stopped when it has been idle. Nothing here touches
//! the UI thread; every call is a message into the process or a channel out of it.
use super::rpc;
use super::servers;
use super::uri;
use async_channel::{Receiver, Sender};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a request may wait for its answer.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

struct Pending {
    reply: Sender<Result<Value, String>>,
    deadline: Instant,
}

pub struct Client {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<i64, Pending>>,
    /// The `initialize` answer, taken once by the first request.
    init: Mutex<Option<Receiver<Result<Value, String>>>>,
    ready: AtomicBool,
    next_id: AtomicI64,
    alive: AtomicBool,
    last_use: Mutex<Instant>,
    opened: Mutex<HashSet<String>>,
}

impl Client {
    /// Start `def`'s server in `root` and send `initialize` without waiting.
    pub fn create(def: &servers::ServerDef, root: Option<&Path>) -> Result<Arc<Client>, String> {
        let mut command = Command::new(def.command);
        command
            .args(def.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(root) = root {
            command.current_dir(root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", def.command))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;

        let client = Arc::new(Client {
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending: Mutex::new(HashMap::new()),
            init: Mutex::new(None),
            ready: AtomicBool::new(false),
            next_id: AtomicI64::new(1),
            alive: AtomicBool::new(true),
            last_use: Mutex::new(Instant::now()),
            opened: Mutex::new(HashSet::new()),
        });

        spawn_reader(client.clone(), stdout);
        spawn_janitor(client.clone());

        let init = client.request("initialize", initialize_params(root))?;
        *client.init.lock().unwrap() = Some(init);
        Ok(client)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Wait for `initialize` to finish and finish the handshake.
    // ponytail: the first request awaits the handshake, a concurrent second one
    // skips it; upgrade: share one init Task between callers.
    pub async fn ensure_ready(&self) -> Result<(), String> {
        let init = self.init.lock().unwrap().take();
        if let Some(rx) = init {
            rx.recv()
                .await
                .map_err(|_| "server closed during initialize".to_string())??;
            let _ = self.notify("initialized", json!({}));
            self.ready.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Has the server already been told about `path`?
    pub fn is_open(&self, path: &Path) -> bool {
        self.opened
            .lock()
            .unwrap()
            .contains(&uri::path_to_uri(path))
    }

    /// Tell the server about `path` once, with the text it will see.
    pub fn did_open(&self, path: &Path, language_id: &str, text: &str) -> Result<(), String> {
        let uri = uri::path_to_uri(path);
        if !self.opened.lock().unwrap().insert(uri.clone()) {
            return Ok(());
        }
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": 1,
                "text": text,
            }}),
        )
    }

    /// Send a request; the answer arrives on the returned channel.
    pub fn request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Receiver<Result<Value, String>>, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = async_channel::bounded(1);
        self.pending.lock().unwrap().insert(
            id,
            Pending {
                reply: tx,
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        if let Err(e) =
            self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
        {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        Ok(rx)
    }

    pub fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    fn write(&self, message: Value) -> Result<(), String> {
        *self.last_use.lock().unwrap() = Instant::now();
        let mut guard = self.stdin.lock().unwrap();
        let stdin = guard.as_mut().ok_or("the server is not running")?;
        stdin
            .write_all(&rpc::encode(&message))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("lost the server's stdin: {e}"))
    }

    /// Stop the process; safe to call more than once.
    pub fn kill(&self) {
        if !self.alive.swap(false, Ordering::SeqCst) {
            return;
        }
        drop(self.stdin.lock().unwrap().take());
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.fail_pending("the server was stopped");
    }

    fn fail_pending(&self, reason: &str) {
        let mut pending = self.pending.lock().unwrap();
        for (_, p) in pending.drain() {
            let _ = p.reply.send_blocking(Err(reason.to_string()));
        }
    }
}

#[cfg(test)]
impl Client {
    /// The handshake and one request, waited out without an async runtime.
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        if let Some(init) = self.init.lock().unwrap().take() {
            init.recv_blocking()
                .map_err(|_| "server closed during initialize".to_string())??;
            let _ = self.notify("initialized", json!({}));
        }
        self.request(method, params)?
            .recv_blocking()
            .map_err(|_| "server closed".to_string())?
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.kill();
    }
}

fn initialize_params(root: Option<&Path>) -> Value {
    let folder = root.map(|p| {
        json!({"uri": uri::path_to_uri(p), "name": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()})
    });
    json!({
        "processId": std::process::id(),
        "clientInfo": {"name": "smithy", "version": env!("CARGO_PKG_VERSION")},
        "rootUri": root.map(uri::path_to_uri),
        "workspaceFolders": folder.map(|f| vec![f]),
        "capabilities": {
            "workspace": {"workspaceFolders": true},
            "textDocument": {
                "definition": {"dynamicRegistration": false, "linkSupport": false},
                "references": {"dynamicRegistration": false}
            }
        }
    })
}

mod pipe;
#[cfg(test)]
mod tests;
use pipe::{spawn_janitor, spawn_reader};
