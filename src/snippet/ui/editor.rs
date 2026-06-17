pub struct EditorPopup;

impl Default for EditorPopup {
    fn default() -> Self {
        Self
    }
}

impl eframe::App for EditorPopup {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Snippet Editor - placeholder");
        });
    }
}
