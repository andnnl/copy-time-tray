pub struct SearchPopup;

impl Default for SearchPopup {
    fn default() -> Self {
        Self
    }
}

impl eframe::App for SearchPopup {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Snippet Search - placeholder");
        });
    }
}
