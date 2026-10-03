//! What a keystroke sends to the pty. Pure, so the mapping is tested without one.

use gpui::Modifiers;

/// The bytes one keystroke writes, or `None` when the terminal does not use the key.
pub fn encode(key: &str, key_char: Option<&str>, mods: Modifiers) -> Option<Vec<u8>> {
    if mods.platform {
        return None;
    }
    if mods.control {
        // Ctrl+letter is the matching C0 code; nothing else is claimed.
        return ctrl_letter(key).map(|b| vec![b]);
    }
    if let Some(bytes) = named(key) {
        return Some(bytes.to_vec());
    }
    if mods.alt {
        // Alt+char is Meta: ESC then the char.
        let ch = printable(key, key_char)?;
        let mut out = vec![0x1b];
        out.extend_from_slice(ch.as_bytes());
        return Some(out);
    }
    printable(key, key_char).map(|ch| ch.as_bytes().to_vec())
}

/// Ctrl+A through Ctrl+Z as 1..26.
fn ctrl_letter(key: &str) -> Option<u8> {
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() || !c.is_ascii_alphabetic() {
        return None;
    }
    Some(c.to_ascii_lowercase() as u8 - b'a' + 1)
}

/// The text a printable key produces, or `None` for control keys.
fn printable<'a>(key: &str, key_char: Option<&'a str>) -> Option<&'a str> {
    let ch = match (key_char, key) {
        (Some(ch), _) => ch,
        // Windows reports the space bar as key "space" with no key_char.
        (None, "space") => " ",
        _ => return None,
    };
    (!ch.chars().any(char::is_control)).then_some(ch)
}

/// The fixed sequences for the named keys: a shell's line editor reads these.
fn named(key: &str) -> Option<&'static [u8]> {
    Some(match key {
        "enter" => b"\r",
        "backspace" => b"\x7f",
        "tab" => b"\t",
        "escape" => b"\x1b",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "insert" => b"\x1b[2~",
        "delete" => b"\x1b[3~",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn ctrl() -> Modifiers {
        Modifiers {
            control: true,
            ..Modifiers::default()
        }
    }

    #[test]
    fn printable_characters_go_through_as_utf8() {
        assert_eq!(
            encode("a", Some("a"), Modifiers::default()),
            Some(b"a".to_vec())
        );
        assert_eq!(
            encode("space", None, Modifiers::default()),
            Some(b" ".to_vec())
        );
        assert_eq!(
            encode("e", Some("é"), Modifiers::default()),
            Some("é".as_bytes().to_vec())
        );
    }

    #[test]
    fn ctrl_letters_become_control_codes() {
        assert_eq!(encode("c", None, ctrl()), Some(vec![3]));
        assert_eq!(encode("D", None, ctrl()), Some(vec![4]));
        assert_eq!(encode("`", Some("`"), ctrl()), None);
    }

    #[test]
    fn named_keys_use_the_standard_sequences() {
        assert_eq!(
            encode("enter", None, Modifiers::default()),
            Some(vec![b'\r'])
        );
        assert_eq!(
            encode("backspace", None, Modifiers::default()),
            Some(vec![0x7f])
        );
        assert_eq!(
            encode("up", None, Modifiers::default()),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            encode("pageup", None, Modifiers::default()),
            Some(b"\x1b[5~".to_vec())
        );
    }

    #[test]
    fn alt_prepends_escape_and_unknown_keys_are_ignored() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(encode("x", Some("x"), alt), Some(b"\x1bx".to_vec()));
        assert_eq!(encode("shift", None, Modifiers::default()), None);
        assert_eq!(encode("capslock", None, Modifiers::default()), None);
    }
}
