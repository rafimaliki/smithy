//! Bars above an editor that need a decision: a file changed on disk and has
//! unsaved edits, or a large file opened read-only. Also the binary-file screen.
//! Layouts come from the framery frames `file-changed`, `large-file`, `binary-file`.
use super::Workspace;
use crate::editor::view::EditorView;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, Entity, FontWeight, Hsla, SharedString, Stateful,
};

/// The bar for the active editor, if it needs one.
pub(super) fn editor_bar(
    editor: &Entity<EditorView>,
    t: &Theme,
    cx: &mut Context<Workspace>,
) -> Option<AnyElement> {
    let view = editor.read(cx);
    if view.disk_changed() {
        let name = file_name(view.state.path.as_deref());
        let reload = {
            let editor = editor.clone();
            banner_button("disk-reload", "Reload", t).on_click(cx.listener(move |_, _, _, cx| {
                editor.update(cx, |editor, cx| editor.reload_from_disk(cx))
            }))
        };
        let keep =
            {
                let editor = editor.clone();
                banner_button("disk-keep", "Keep mine", t).on_click(cx.listener(
                    move |_, _, _, cx| editor.update(cx, |editor, cx| editor.keep_mine(cx)),
                ))
            };
        let text: SharedString =
            format!("{name} changed on disk, and you have unsaved edits here.").into();
        return Some(bar(
            t,
            text,
            vec![reload.into_any_element(), keep.into_any_element()],
        ));
    }
    if view.state.read_only {
        let name = file_name(view.state.path.as_deref());
        let size = size_text(view.size_bytes());
        let open = {
            let editor = editor.clone();
            banner_button("open-normally", "Open normally", t).on_click(cx.listener(
                move |_, _, _, cx| editor.update(cx, |editor, cx| editor.open_normally(cx)),
            ))
        };
        let text: SharedString =
            format!("{name} is {size}. Opened read-only without highlighting to save memory.")
                .into();
        return Some(bar(t, text, vec![open.into_any_element()]));
    }
    None
}

/// Binary files are never loaded; this explains why and offers Reveal.
pub(super) fn binary_screen(
    t: &Theme,
    path: &std::path::Path,
    size: u64,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let name = file_name(Some(path));
    let size = size_text(size);
    let target = path.to_path_buf();
    let reveal = banner_button("reveal-binary", "Reveal in File Explorer", t)
        .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&target)));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .text_center()
        .text_color(t.mute)
        .child(
            div()
                .text_color(t.ink)
                .font_weight(FontWeight::SEMIBOLD)
                .mb(px(4.))
                .child(format!("{name} is a binary file")),
        )
        .child(SharedString::from(format!(
            "{size}. It is not shown in the editor. Images, SVG and PDF open in viewers."
        )))
        .child(div().mt(px(14.)).child(reveal))
        .into_any_element()
}

fn bar(t: &Theme, text: SharedString, buttons: Vec<AnyElement>) -> AnyElement {
    let mut tint: Hsla = t.acc.into();
    tint.a = 0.12;
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(38.))
        .px(px(16.))
        .bg(tint)
        .border_b_1()
        .border_color(t.line)
        .text_color(t.ink)
        .child(div().flex_1().child(text))
        .children(buttons)
        .into_any_element()
}

fn banner_button(id: &'static str, label: &'static str, t: &Theme) -> Stateful<gpui::Div> {
    let mut border: Hsla = t.ink.into();
    border.a = 0.22;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .text_color(t.ink)
        .hover(|d| d.bg(t.hov))
        .child(label)
}

fn file_name(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Human size for the bars: megabytes above one, kilobytes below.
fn size_text(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else {
        format!("{} KB", bytes.div_ceil(1024).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::size_text;

    #[test]
    fn size_text_switches_units_at_a_megabyte() {
        assert_eq!(size_text(0), "1 KB");
        assert_eq!(size_text(9000), "9 KB");
        assert_eq!(size_text(1024 * 1024 - 1), "1024 KB");
        assert_eq!(size_text(1024 * 1024), "1 MB");
        assert_eq!(size_text(2_753_015), "2 MB");
    }
}
