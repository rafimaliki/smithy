//! The server's pipes: a reader thread that routes answers to whoever asked, and
//! a janitor that stops a server which has had nothing to do.
use super::rpc::Decoder;
use super::Client;
use serde_json::{json, Value};
use std::io::{BufReader, Read};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A server with nothing to do for this long is stopped.
pub const IDLE: Duration = Duration::from_secs(180);
/// How often the janitor looks.
const TICK: Duration = Duration::from_secs(5);

pub(super) fn spawn_reader(client: Arc<Client>, stdout: std::process::ChildStdout) {
    std::thread::spawn(move || {
        let mut decoder = Decoder::new();
        let mut reader = BufReader::new(stdout);
        let mut chunk = [0u8; 8192];
        loop {
            let n = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            decoder.push(&chunk[..n]);
            while let Some(message) = decoder.next_message() {
                if let Ok(Value::Object(object)) = message {
                    handle(&client, object);
                }
            }
        }
        client.alive.store(false, Ordering::SeqCst);
        client.fail_pending("the server exited");
    });
}

/// A response goes to whoever asked; a request from the server gets a plain
/// answer so it is not left waiting.
fn handle(client: &Arc<Client>, mut object: serde_json::Map<String, Value>) {
    let Some(id) = object.get("id").and_then(Value::as_i64) else {
        return; // a notification
    };
    if object.contains_key("method") {
        let result =
            if object.get("method").and_then(Value::as_str) == Some("workspace/configuration") {
                json!([])
            } else {
                Value::Null
            };
        let _ = client.write(json!({"jsonrpc": "2.0", "id": id, "result": result}));
        return;
    }
    let pending = client.pending.lock().unwrap().remove(&id);
    let Some(pending) = pending else {
        return;
    };
    let outcome = match object.remove("error") {
        Some(Value::Null) | None => Ok(object.remove("result").unwrap_or(Value::Null)),
        Some(error) => Err(error_message(&error)),
    };
    let _ = pending.reply.send_blocking(outcome);
}

fn error_message(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("the server rejected the request");
    match error.get("data").and_then(Value::as_str) {
        Some(data) => format!("{message}: {data}"),
        None => message.to_string(),
    }
}

pub(super) fn spawn_janitor(client: Arc<Client>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        if !client.is_alive() {
            break;
        }
        let now = Instant::now();
        let expired: Vec<i64> = {
            let pending = client.pending.lock().unwrap();
            pending
                .iter()
                .filter(|(_, p)| p.deadline <= now)
                .map(|(id, _)| *id)
                .collect()
        };
        for id in expired {
            if let Some(p) = client.pending.lock().unwrap().remove(&id) {
                let _ = p
                    .reply
                    .send_blocking(Err("the server did not answer in time".into()));
            }
        }
        let idle = now.duration_since(*client.last_use.lock().unwrap());
        let quiet = client.pending.lock().unwrap().is_empty();
        if should_stop(idle, quiet) {
            client.kill();
            break;
        }
    });
}

/// A server that has had nothing to do for [`IDLE`] is stopped; one with a
/// request still open is left alone.
pub(super) fn should_stop(idle: Duration, quiet: bool) -> bool {
    idle > IDLE && quiet
}
