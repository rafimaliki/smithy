//! Pure helpers for the image viewer: the bar's dimension and file-size text, an
//! SVG's declared size, and the zoom ladder. No gpui here, so it is unit-tested.

/// A pixel size, as declared by an SVG or decoded from a raster image.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Dimensions {
    pub width: f32,
    pub height: f32,
}

impl Dimensions {
    /// `300 × 300` — the bar's dimension text: at most one decimal, none when whole.
    pub fn bar_text(self) -> String {
        format!("{} × {}", round1(self.width), round1(self.height))
    }

    /// This size at `percent` zoom, as a pixel size.
    pub fn scaled(self, percent: f32) -> (f32, f32) {
        (self.width * percent / 100.0, self.height * percent / 100.0)
    }
}

fn round1(n: f32) -> String {
    let r = (n * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 0.05 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r:.1}")
    }
}

/// `512 B`, `1.2 KB`, `38 KB`, `2.5 MB` — the bar's file size.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    let (value, unit) = if b < KB {
        return format!("{bytes} B");
    } else if b < MB {
        (b / KB, "KB")
    } else {
        (b / MB, "MB")
    };
    if value < 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

/// Percent steps the zoom buttons walk.
const LADDER: [f32; 11] = [
    10.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 300.0, 400.0, 800.0, 1600.0,
];

/// The next percent above (or below) `current`; unchanged at the ends.
pub fn zoom_step(current: f32, up: bool) -> f32 {
    if up {
        LADDER
            .iter()
            .copied()
            .find(|&z| z > current)
            .unwrap_or(current)
    } else {
        LADDER
            .iter()
            .rev()
            .copied()
            .find(|&z| z < current)
            .unwrap_or(current)
    }
}

/// The width and height an SVG declares, in px. `width`/`height` when both parse,
/// otherwise `viewBox`; one declared side takes the other's ratio from the viewBox.
/// `None` when the file declares neither (or is not an SVG).
pub fn svg_dimensions(text: &str) -> Option<Dimensions> {
    let tag = root_tag(text)?;
    let width = attr(tag, "width").and_then(length_px);
    let height = attr(tag, "height").and_then(length_px);
    if let (Some(width), Some(height)) = (width, height) {
        return Some(Dimensions { width, height });
    }
    let (view_w, view_h) = attr(tag, "viewBox").and_then(view_box)?;
    Some(match (width, height) {
        (Some(width), None) => Dimensions {
            width,
            height: width * view_h / view_w,
        },
        (None, Some(height)) => Dimensions {
            width: height * view_w / view_h,
            height,
        },
        _ => Dimensions {
            width: view_w,
            height: view_h,
        },
    })
}

/// The root `<svg …>` tag, from `<svg` to its closing `>`.
fn root_tag(text: &str) -> Option<&str> {
    let start = text.find("<svg")?;
    let rest = &text[start..];
    if !rest["<svg".len()..].starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace()) {
        return None;
    }
    tag_end(rest).map(|end| &rest[..end])
}

/// Index of the `>` that closes a tag, ignoring a `>` inside a quoted value.
fn tag_end(tag: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in tag.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '>' => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// The value of `name` in a tag, or `None`. Attribute names are case-sensitive.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag.strip_prefix("<svg")?;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() || rest.starts_with('>') {
            return None;
        }
        let (key, after) = rest.split_at(
            rest.find(|c: char| c.is_whitespace() || c == '=')
                .unwrap_or(rest.len()),
        );
        let Some(after) = after.trim_start().strip_prefix('=') else {
            // No `=`: not an attribute. Skip it and read the next one.
            rest = &rest[key.len()..];
            continue;
        };
        let (value, next) = read_value(after.trim_start());
        if key == name {
            return Some(value);
        }
        rest = next;
    }
}

/// A quoted or bare attribute value, and what follows it.
fn read_value(after: &str) -> (&str, &str) {
    match after.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let body = &after[1..];
            match body.find(quote) {
                Some(end) => (&body[..end], &body[end + 1..]),
                None => (body, ""),
            }
        }
        _ => {
            let end = after.find(char::is_whitespace).unwrap_or(after.len());
            after.split_at(end)
        }
    }
}

/// A CSS length in px. `%` and unknown units do not resolve to a fixed size.
fn length_px(value: &str) -> Option<f32> {
    let value = value.trim();
    let end = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(end);
    let number: f32 = number.parse().ok()?;
    let scale = match unit.trim() {
        "" | "px" => 1.0,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        "mm" => 96.0 / 25.4,
        "cm" => 96.0 / 2.54,
        "in" => 96.0,
        _ => return None,
    };
    Some(number * scale)
}

/// The `width height` of a `viewBox="minX minY width height"`.
fn view_box(value: &str) -> Option<(f32, f32)> {
    let numbers: Vec<f32> = value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let (w, h) = (*numbers.get(2)?, *numbers.get(3)?);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_the_bar_shows_them() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1228), "1.2 KB");
        assert_eq!(format_size(38_912), "38 KB");
        assert_eq!(format_size(2_600_000), "2.5 MB");
        assert_eq!(format_size(15 * 1024 * 1024), "15 MB");
    }

    #[test]
    fn dimensions_text_drops_a_whole_decimal() {
        assert_eq!(
            Dimensions {
                width: 300.0,
                height: 300.0
            }
            .bar_text(),
            "300 × 300"
        );
        assert_eq!(
            Dimensions {
                width: 12.5,
                height: 8.0
            }
            .bar_text(),
            "12.5 × 8"
        );
    }

    #[test]
    fn zoom_walks_the_ladder_and_stops_at_the_ends() {
        assert_eq!(zoom_step(100.0, true), 150.0);
        assert_eq!(zoom_step(100.0, false), 75.0);
        assert_eq!(zoom_step(120.0, true), 150.0);
        assert_eq!(zoom_step(120.0, false), 100.0);
        assert_eq!(zoom_step(10.0, false), 10.0);
        assert_eq!(zoom_step(1600.0, true), 1600.0);
    }

    #[test]
    fn svg_dimensions_come_from_width_and_height() {
        let svg = r#"<?xml version="1.0"?>
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 48 48">
              <rect width="48" height="48"/>
            </svg>"#;
        assert_eq!(
            svg_dimensions(svg),
            Some(Dimensions {
                width: 24.0,
                height: 24.0
            })
        );
    }

    #[test]
    fn svg_dimensions_fall_back_to_the_viewbox_and_keep_units_in_px() {
        let no_size = r#"<svg viewBox="0 0 300 150"><rect/></svg>"#;
        assert_eq!(
            svg_dimensions(no_size),
            Some(Dimensions {
                width: 300.0,
                height: 150.0
            })
        );

        let units = r#"<svg width="10mm" height="1in"><rect/></svg>"#;
        let dims = svg_dimensions(units).unwrap();
        assert!((dims.width - 37.795).abs() < 0.01);
        assert_eq!(dims.height, 96.0);

        // A single declared side takes the viewBox ratio for the other.
        let one_side = r#"<svg width="100" viewBox="0 0 200 50"><rect/></svg>"#;
        assert_eq!(
            svg_dimensions(one_side),
            Some(Dimensions {
                width: 100.0,
                height: 25.0
            })
        );
    }

    #[test]
    fn svg_dimensions_ignore_the_wrong_attributes_and_non_svg_files() {
        assert_eq!(svg_dimensions("<html><body/></html>"), None);
        assert_eq!(svg_dimensions("<svg width=\"100%\"><rect/></svg>"), None);
        // A `stroke-width` is not a `width`, and a `>` inside a value does not end the tag.
        assert_eq!(
            svg_dimensions(r#"<svg stroke-width="4" viewBox="0 0 8 4" data-note="a>b"/>"#),
            Some(Dimensions {
                width: 8.0,
                height: 4.0
            })
        );
        assert_eq!(
            svg_dimensions(r#"<svg width = '12' height = '6'/>"#),
            Some(Dimensions {
                width: 12.0,
                height: 6.0
            })
        );
    }
}
