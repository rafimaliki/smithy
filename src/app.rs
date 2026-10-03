//! Application start-up: settings, key bindings, the main window.
use crate::keymap;
use crate::settings::Settings;
use crate::tree::ops::Clipboard;
use crate::workspace::Workspace;
use gpui::{
    px, size, App, AppContext as _, Application, Bounds, Focusable, TitlebarOptions, WindowBounds,
    WindowOptions,
};

pub fn run() {
    Application::new()
        .with_assets(crate::assets::Assets)
        .run(|cx: &mut App| {
            let settings = Settings::load();
            cx.bind_keys(keymap::bindings(&settings.shortcuts));
            cx.set_global(settings);
            cx.set_global(Clipboard::default());
            cx.open_window(window_options(cx), |window, cx| {
                let workspace = cx.new(Workspace::new);
                window.focus(&workspace.focus_handle(cx));
                // `smithy <folder>` opens that folder straight away.
                if let Some(dir) = std::env::args_os().nth(1) {
                    workspace.update(cx, |ws, cx| ws.open_folder(dir.into(), window, cx));
                }
                workspace
            })
            .expect("open main window");
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}

/// 1280x800, shrunk to 90% of the primary display when that is smaller, so the
/// window (and its status bar) always fits on screen.
fn window_options(cx: &App) -> WindowOptions {
    let mut want = size(px(1280.), px(800.));
    if let Some(display) = cx.primary_display() {
        let avail = display.bounds().size;
        want.width = want.width.min(avail.width * 0.9);
        want.height = want.height.min(avail.height * 0.9);
    }
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, want, cx))),
        window_min_size: Some(size(px(640.), px(420.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Smithy".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}
