// Throwaway spike: does gpui open a window and scroll 50k lines on Windows?
use gpui::{
    div, prelude::*, px, rgb, uniform_list, App, Application, Context, Window, WindowOptions,
};

struct Lines;

impl Render for Lines {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x2e3440))
            .text_color(rgb(0xd8dee9))
            .child(
                uniform_list(
                    "lines",
                    50_000,
                    cx.processor(|_this, range: std::ops::Range<usize>, _window, _cx| {
                        range
                            .map(|i| {
                                div()
                                    .h(px(20.))
                                    .child(format!("{:>6}  let line_{i} = {i} * 2;", i + 1))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full(),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Lines))
            .unwrap();
        cx.activate(true);
    });
}
