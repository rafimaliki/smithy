//! The rendered side of the Markdown add-on: the block tree into gpui elements.
//! Sizes and colors follow the `markdown-preview` frame and the `Theme` tokens.
use super::parse::{Block, Item, Span};
use crate::editor::view::MONO;
use crate::theme::Theme;
use gpui::{
    div, img, prelude::*, px, rems, AnyElement, FontStyle, FontWeight, HighlightStyle,
    SharedString, StrikethroughStyle, StyledText, UnderlineStyle,
};
use std::ops::Range;
use std::path::{Path, PathBuf};

const PROSE: f32 = 13.0;
const INLINE_CODE: f32 = 12.5;
const LINE: f32 = 1.65;

/// The scrollable rendered document (`.mdr` on the board).
// ponytail: every block is laid out each frame; a huge document would want a
// virtualized list of blocks. Upgrade: `uniform_list` over the top-level blocks.
pub fn document(doc: &Path, blocks: &[Block], t: &Theme) -> AnyElement {
    div()
        .id("markdown-preview")
        .size_full()
        .overflow_y_scroll()
        .font_family("Segoe UI")
        .text_size(px(PROSE))
        .line_height(rems(LINE))
        .text_color(t.ink)
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(40.))
                .py(px(22.))
                .children(blocks_of(doc, blocks, t)),
        )
        .into_any_element()
}

fn blocks_of(doc: &Path, blocks: &[Block], t: &Theme) -> Vec<AnyElement> {
    blocks.iter().map(|b| block(doc, b, t)).collect()
}

fn block(doc: &Path, block: &Block, t: &Theme) -> AnyElement {
    match block {
        Block::Heading { level, spans } => heading(*level, spans, t),
        Block::Paragraph(spans) => paragraph(doc, spans, t),
        Block::Code(text) => div()
            .bg(t.side)
            .border_1()
            .border_color(t.line)
            .rounded(px(6.))
            .px(px(14.))
            .py(px(12.))
            .mb(px(12.))
            .font_family(MONO)
            .text_size(px(INLINE_CODE))
            .line_height(rems(1.6))
            .child(SharedString::from(text.clone()))
            .into_any_element(),
        Block::List {
            ordered,
            start,
            items,
        } => list(doc, *ordered, *start, items, t),
        Block::Table { head, rows } => table(head, rows, t),
        Block::Quote(inner) => div()
            .flex()
            .flex_col()
            .mb(px(12.))
            .pl(px(12.))
            .border_l_2()
            .border_color(t.acc)
            .text_color(t.mute)
            .children(blocks_of(doc, inner, t))
            .into_any_element(),
        Block::Rule => div()
            .w_full()
            .h(px(1.))
            .mb(px(12.))
            .bg(t.line)
            .into_any_element(),
    }
}

fn heading(level: u8, spans: &[Span], t: &Theme) -> AnyElement {
    let (size, weight) = match level {
        1 => (26.0, FontWeight::SEMIBOLD),
        2 => (18.0, FontWeight::SEMIBOLD),
        3 => (15.0, FontWeight::SEMIBOLD),
        _ => (PROSE, FontWeight::SEMIBOLD),
    };
    let mut d = div()
        .text_size(px(size))
        .font_weight(weight)
        .text_color(t.ink);
    d = match level {
        1 => d.pb(px(8.)).mb(px(12.)).border_b_1().border_color(t.line),
        2 => d.mt(px(22.)).mb(px(8.)),
        _ => d.mt(px(16.)).mb(px(6.)),
    };
    d.child(text(spans, t)).into_any_element()
}

fn paragraph(doc: &Path, spans: &[Span], t: &Theme) -> AnyElement {
    // A paragraph that is one image is that image, not its alt text.
    if let [span] = spans {
        if span.style.image {
            return image(doc, span, t);
        }
    }
    div().mb(px(10.)).child(text(spans, t)).into_any_element()
}

/// One styled text element, so the whole paragraph wraps as one run of prose.
fn text(spans: &[Span], t: &Theme) -> AnyElement {
    let mut buf = String::new();
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::with_capacity(spans.len());
    for span in spans {
        let start = buf.len();
        buf.push_str(&span.text);
        highlights.push((start..buf.len(), highlight(span, t)));
    }
    StyledText::new(buf)
        .with_highlights(highlights)
        .into_any_element()
}

fn highlight(span: &Span, t: &Theme) -> HighlightStyle {
    let mut h = HighlightStyle::default();
    if span.style.bold {
        h.font_weight = Some(FontWeight::BOLD);
    }
    if span.style.italic {
        h.font_style = Some(FontStyle::Italic);
    }
    if span.style.strike {
        h.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        });
    }
    if span.style.code {
        h.color = Some(t.str.into());
        h.background_color = Some(t.side.into());
    }
    if span.url.is_some() && !span.style.image {
        h.color = Some(t.acc.into());
        h.underline = Some(UnderlineStyle {
            thickness: px(1.),
            color: Some(t.acc.into()),
            wavy: false,
        });
    }
    if span.style.image {
        h.color = Some(t.mute.into());
    }
    h
}

/// An image block. Only files next to the document are drawn; a remote or missing
/// source keeps its alt text. ponytail: no network images, no percent-decoding;
/// upgrade: gpui's http client for `https:` sources.
fn image(doc: &Path, span: &Span, t: &Theme) -> AnyElement {
    let src = span.url.as_deref().unwrap_or_default();
    if let Some(path) = local_image(doc, src) {
        return div()
            .mb(px(12.))
            .child(img(path).max_w_full())
            .into_any_element();
    }
    let alt = if span.text.is_empty() {
        src
    } else {
        &span.text
    };
    div()
        .mb(px(12.))
        .px(px(10.))
        .py(px(6.))
        .border_1()
        .border_color(t.line)
        .rounded(px(6.))
        .text_color(t.mute)
        .child(SharedString::from(format!("[image: {alt}]")))
        .into_any_element()
}

fn local_image(doc: &Path, src: &str) -> Option<PathBuf> {
    if src.is_empty() || src.contains("://") || src.starts_with("data:") {
        return None;
    }
    let path = Path::new(src);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        doc.parent()?.join(path)
    };
    path.is_file().then_some(path)
}

fn list(doc: &Path, ordered: bool, start: u64, items: &[Item], t: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .mb(px(12.))
        .pl(px(22.))
        .children(
            items
                .iter()
                .enumerate()
                .map(|(i, item)| list_item(doc, ordered, start + i as u64, item, t)),
        )
        .into_any_element()
}

fn list_item(doc: &Path, ordered: bool, number: u64, item: &Item, t: &Theme) -> AnyElement {
    let marker: SharedString = match item.checked {
        Some(true) => "☑".into(),
        Some(false) => "☐".into(),
        None if ordered => format!("{number}.").into(),
        None => "•".into(),
    };
    let row = div()
        .flex()
        .items_start()
        .child(
            div()
                .w(px(18.))
                .flex_none()
                .text_color(t.mute)
                .child(marker),
        )
        .child(div().flex_1().min_w_0().child(text(&item.spans, t)));
    let mut col = div().flex().flex_col().child(row);
    if !item.blocks.is_empty() {
        col = col.child(div().pl(px(18.)).children(blocks_of(doc, &item.blocks, t)));
    }
    col.into_any_element()
}

fn table(head: &[Vec<Span>], rows: &[Vec<Vec<Span>>], t: &Theme) -> AnyElement {
    let mut table = div()
        .flex()
        .flex_col()
        .mb(px(12.))
        .border_t_1()
        .border_l_1()
        .border_color(t.line);
    if !head.is_empty() {
        table = table.child(table_row(head, true, t));
    }
    for row in rows {
        table = table.child(table_row(row, false, t));
    }
    table.into_any_element()
}

fn table_row(cells: &[Vec<Span>], header: bool, t: &Theme) -> AnyElement {
    div()
        .flex()
        .children(cells.iter().map(|cell| table_cell(cell, header, t)))
        .into_any_element()
}

fn table_cell(spans: &[Span], header: bool, t: &Theme) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .px(px(12.))
        .py(px(5.))
        .border_r_1()
        .border_b_1()
        .border_color(t.line)
        .when(header, |d| d.bg(t.side).font_weight(FontWeight::SEMIBOLD))
        .child(text(spans, t))
        .into_any_element()
}
