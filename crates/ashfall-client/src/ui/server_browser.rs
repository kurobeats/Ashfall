//! Server browser — master server query and display.
//! ponytail: stubs — full master query in Phase 8.

use eframe::egui;

pub struct ServerBrowser;

impl ServerBrowser {
    pub fn new() -> Self { ServerBrowser }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if ui.button("Refresh").clicked() {}
        ui.label("No servers found. Enter address to connect directly.");
    }
}
