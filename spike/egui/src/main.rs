// Throwaway spike: same test as spike/gpui, for a RAM and start-time comparison.
use eframe::egui;

struct Lines;

impl eframe::App for Lines {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show_rows(ui, 20.0, 50_000, |ui, range| {
                for i in range {
                    ui.monospace(format!("{:>6}  let line_{i} = {i} * 2;", i + 1));
                }
            });
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "smithy-spike-egui",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(Lines))),
    )
}
