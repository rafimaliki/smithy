//! Semantic color tokens. Values come from the framery board, page `themes`
//! (table `theme-tokens`); Nord Dark is the default.
use gpui::{rgb, Rgba};

#[derive(Clone, Copy)]
#[allow(dead_code)] // add and the syntax colors are read by the diff and highlighting features
pub struct Theme {
    pub name: &'static str,
    pub bg: Rgba,
    pub side: Rgba,
    pub rail: Rgba,
    pub line: Rgba,
    pub ink: Rgba,
    pub mute: Rgba,
    pub acc: Rgba,
    pub sel: Rgba,
    pub hov: Rgba,
    pub add: Rgba,
    pub del: Rgba,
    /// Syntax: keyword, string, function, type, comment.
    pub kw: Rgba,
    pub str: Rgba,
    pub func: Rgba,
    pub ty: Rgba,
    pub comment: Rgba,
}

const fn c(v: u32) -> Rgba {
    // `rgb()` is not const; build the same value by hand.
    Rgba {
        r: ((v >> 16) & 0xff) as f32 / 255.0,
        g: ((v >> 8) & 0xff) as f32 / 255.0,
        b: (v & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

// ponytail: syntax colors are shared per light/dark family, hover reuses the
// selection color for non-default themes; upgrade: per-theme values on the board.
pub const THEMES: [Theme; 4] = [
    Theme {
        name: "Nord Dark",
        bg: c(0x131517),
        side: c(0x1a1b1d),
        rail: c(0x1a1b1d),
        line: c(0x27282b),
        ink: c(0xdbdbdb),
        mute: c(0x80848a),
        acc: c(0x70949e),
        sel: c(0x26282b),
        hov: c(0x222224),
        add: c(0x81b88b),
        del: c(0xc74e39),
        kw: c(0x8ba0b6),
        str: c(0xa8bc96),
        func: c(0x98bdc8),
        ty: c(0x9fb9b8),
        comment: c(0x515660),
    },
    Theme {
        name: "Warm graphite",
        bg: c(0x2a2826),
        side: c(0x232120),
        rail: c(0x1d1c1b),
        line: c(0x37342f),
        ink: c(0xd8d3ca),
        mute: c(0x8b857a),
        acc: c(0xe0a458),
        sel: c(0x38342e),
        hov: c(0x38342e),
        add: c(0x7cb96a),
        del: c(0xe07a6b),
        kw: c(0xe0a458),
        str: c(0xa8bc96),
        func: c(0x98bdc8),
        ty: c(0x9fb9b8),
        comment: c(0x6f695f),
    },
    Theme {
        name: "Midnight indigo",
        bg: c(0x1f2133),
        side: c(0x1a1c2c),
        rail: c(0x161826),
        line: c(0x2b2e45),
        ink: c(0xc9cce3),
        mute: c(0x7c80a0),
        acc: c(0x8f8cf7),
        sel: c(0x2c2f4a),
        hov: c(0x2c2f4a),
        add: c(0x73c991),
        del: c(0xf7768e),
        kw: c(0x8f8cf7),
        str: c(0xa8bc96),
        func: c(0x7dcfff),
        ty: c(0x9fb9b8),
        comment: c(0x545877),
    },
    Theme {
        name: "Daylight",
        bg: c(0xfbfaf8),
        side: c(0xf2f0ec),
        rail: c(0xe9e6e0),
        line: c(0xdcd8d0),
        ink: c(0x24272e),
        mute: c(0x7a7f89),
        acc: c(0x2f6fdb),
        sel: c(0xe0e6f2),
        hov: c(0xe0e6f2),
        add: c(0x2da44e),
        del: c(0xd1242f),
        kw: c(0x2f6fdb),
        str: c(0x2e7d32),
        func: c(0x00778c),
        ty: c(0x6a4bb5),
        comment: c(0x9aa0a8),
    },
];

impl Theme {
    pub fn by_name(name: &str) -> Theme {
        THEMES
            .iter()
            .find(|t| t.name == name)
            .copied()
            .unwrap_or(THEMES[0])
    }
}

/// Dark text color for use on the accent (badges, buttons).
pub fn on_accent() -> Rgba {
    rgb(0x10131a)
}
