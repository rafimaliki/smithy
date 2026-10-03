//! Tests for the session: a fake server as a real process, and the idle rule.
use super::pipe::{should_stop, IDLE};
use super::Client;
use crate::addon::lsp::servers::ServerDef;
use crate::editor::lang::Lang;
use serde_json::json;
use std::time::Duration;

/// A tiny server: Content-Length frames in, an answer naming the method back.
/// Windows always ships Windows PowerShell, so this needs nothing installed.
const FAKE: &str = r#"
$in = [Console]::OpenStandardInput()
$out = [Console]::OpenStandardOutput()
$reader = New-Object System.IO.StreamReader($in)
while ($true) {
  $len = -1
  while ($true) {
    $line = $reader.ReadLine()
    if ($null -eq $line) { exit }
    if ($line -eq '') { break }
    if ($line.StartsWith('Content-Length:')) { $len = [int]$line.Substring(15).Trim() }
  }
  if ($len -le 0) { continue }
  $buf = New-Object char[] $len
  [void]$reader.ReadBlock($buf, 0, $len)
  $msg = -join $buf
  $id = [regex]::Match($msg, '"id"\s*:\s*(\d+)')
  if (-not $id.Success) { continue }
  $method = [regex]::Match($msg, '"method"\s*:\s*"([^"]+)"').Groups[1].Value
  $body = '{"jsonrpc":"2.0","id":' + $id.Groups[1].Value + ',"result":{"method":"' + $method + '"}}'
  $bytes = [Text.Encoding]::UTF8.GetBytes($body)
  $header = [Text.Encoding]::ASCII.GetBytes("Content-Length: $($bytes.Length)`r`n`r`n")
  $out.Write($header, 0, $header.Length)
  $out.Write($bytes, 0, $bytes.Length)
  $out.Flush()
}
"#;

/// The fake server as a [`ServerDef`], with the script written to a temp dir.
fn fake_server() -> ServerDef {
    let dir = std::env::temp_dir().join("smithy-lsp-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let script = dir.join("fake-server.ps1");
    std::fs::write(&script, FAKE).expect("write fake server");

    let command: &'static str = Box::leak(
        std::env::var("SystemRoot")
            .map(|root| format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe"))
            .unwrap_or_else(|_| "powershell".to_string())
            .into_boxed_str(),
    );
    let script: &'static str = Box::leak(script.to_string_lossy().into_owned().into_boxed_str());
    let args: &'static [&'static str] = Box::leak(Box::new([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        script,
    ]));
    ServerDef {
        id: "fake",
        lang: Lang::Rust,
        language_id: "rust",
        command,
        label: "fake",
        args,
    }
}

#[test]
fn a_request_round_trips_through_a_real_process() {
    let client = match Client::create(&fake_server(), None) {
        Ok(client) => client,
        // No PowerShell on this machine: nothing to test against, and the
        // framing itself is covered by `rpc`'s tests.
        Err(why) => {
            eprintln!("skipping: {why}");
            return;
        }
    };
    let answer = client
        .call("textDocument/definition", json!({}))
        .expect("the fake server answers");
    // The answer names the method: responses were matched by id, not order.
    assert_eq!(answer["method"], "textDocument/definition");
    let second = client
        .call("textDocument/references", json!({}))
        .expect("the fake server answers again");
    assert_eq!(second["method"], "textDocument/references");

    client.kill();
    assert!(!client.is_alive());
    assert!(client.call("textDocument/definition", json!({})).is_err());
}

#[test]
fn a_server_is_only_stopped_when_it_is_both_idle_and_quiet() {
    assert!(!should_stop(IDLE - Duration::from_secs(1), true));
    assert!(!should_stop(IDLE + Duration::from_secs(1), false));
    assert!(should_stop(IDLE + Duration::from_secs(1), true));
}
