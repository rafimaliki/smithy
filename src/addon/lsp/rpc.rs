//! Base-protocol framing for a language server's stdio stream: `Content-Length`
//! headers followed by a JSON body. Pure — bytes in, messages out — so the
//! awkward parts (split chunks, two messages in one read, a bad header) are tested
//! here instead of against a real server.
use serde_json::Value;

/// Frame `message` for the wire: the header, then the JSON body.
pub fn encode(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("a JSON value always encodes");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

/// A byte stream split back into messages. Feed it whatever `read` returned; it
/// keeps the remainder until the rest arrives.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// The next whole message, `None` while more bytes are needed, or an error
    /// for a frame that cannot be trusted (the offending bytes are dropped so the
    /// stream can resync on the next header).
    pub fn next_message(&mut self) -> Option<Result<Value, String>> {
        let (head_end, body_start) = split(&self.buf)?;
        // A stray write in front of the header must not break the whole stream.
        let head_start = find_header(&self.buf[..head_end], 0).unwrap_or(0);
        let length = match content_length(&self.buf[head_start..head_end]) {
            Ok(n) => n,
            Err(e) => {
                let next = find_header(&self.buf, head_start + 1);
                let cut = next
                    .unwrap_or(body_start)
                    .max(head_start + 1)
                    .min(self.buf.len());
                self.buf.drain(..cut);
                return Some(Err(e));
            }
        };
        if self.buf.len() < body_start + length {
            return None;
        }
        let body: Vec<u8> = self.buf[body_start..body_start + length].to_vec();
        self.buf.drain(..body_start + length);
        Some(serde_json::from_slice(&body).map_err(|e| format!("bad JSON body: {e}")))
    }
}

/// Where the header block ends and the body begins, accepting the bare `\n\n`
/// some servers write.
fn split(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, i + 2));
        }
        if i + 3 < buf.len() && &buf[i..i + 4] == b"\r\n\r\n" {
            return Some((i, i + 4));
        }
        i += 1;
    }
    None
}

/// Where `Content-Length` starts, so junk in front of a header can be skipped.
fn find_header(buf: &[u8], from: usize) -> Option<usize> {
    const NEEDLE: &[u8] = b"content-length";
    (from..buf.len().saturating_sub(NEEDLE.len()))
        .find(|&i| buf[i..i + NEEDLE.len()].eq_ignore_ascii_case(NEEDLE))
}

fn content_length(head: &[u8]) -> Result<usize, String> {
    let text = String::from_utf8_lossy(head);
    for line in text.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse()
                .map_err(|_| format!("bad Content-Length: {value}"));
        }
    }
    Err("message without Content-Length".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn decode_all(decoder: &mut Decoder) -> Vec<Value> {
        let mut out = Vec::new();
        while let Some(msg) = decoder.next_message() {
            out.push(msg.expect("well formed"));
        }
        out
    }

    #[test]
    fn round_trip_two_messages_in_one_read() {
        let mut bytes = encode(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}));
        bytes.extend(encode(&json!({"jsonrpc": "2.0", "id": 2, "result": null})));
        let mut d = Decoder::new();
        d.push(&bytes);
        let msgs = decode_all(&mut d);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["id"], 1);
        assert_eq!(msgs[1]["id"], 2);
    }

    #[test]
    fn header_and_body_split_across_reads() {
        let bytes = encode(&json!({"jsonrpc": "2.0", "method": "textDocument/definition"}));
        let mut d = Decoder::new();
        for byte in &bytes {
            d.push(&[*byte]);
        }
        let msgs = decode_all(&mut d);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["method"], "textDocument/definition");
    }

    #[test]
    fn length_is_bytes_not_characters() {
        let bytes = encode(&json!({"text": "héllo — ok"}));
        let mut d = Decoder::new();
        d.push(&bytes);
        let msgs = decode_all(&mut d);
        assert_eq!(msgs[0]["text"], "héllo — ok");
        assert!(d.buf.is_empty());
    }

    #[test]
    fn missing_length_is_an_error_and_the_stream_recovers() {
        let mut d = Decoder::new();
        d.push(b"X-Nope: 1\r\n\r\n{\"id\":1}");
        assert!(d.next_message().unwrap().is_err());
        d.push(&encode(&json!({"id": 2})));
        assert_eq!(d.next_message().unwrap().unwrap()["id"], 2);
    }

    #[test]
    fn bad_json_is_an_error_then_the_next_message_parses() {
        let mut d = Decoder::new();
        d.push(b"Content-Length: 3\r\n\r\n{[]");
        assert!(d.next_message().unwrap().is_err());
        d.push(&encode(&json!({"id": 9})));
        assert_eq!(d.next_message().unwrap().unwrap()["id"], 9);
    }
}
