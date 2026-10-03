//! Application start-up: settings, key bindings, the main window.
use crate::keymap;
use crate::settings::Settings;
use crate::tree::ops::Clipboard;
use crate::workspace::Workspace;
use gpui::{App, AppContext as _, Application, Focusable, WindowOptions};

pub fn run() {
    Application::new()
        .with_assets(crate::assets::Assets)
        .run(|cx: &mut App| {
            let settings = Settings::load();
            cx.bind_keys(keymap::bindings(&settings.shortcuts));
            cx.set_global(settings);
            cx.set_global(Clipboard::default());
            cx.open_window(WindowOptions::default(), |window, cx| {
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
