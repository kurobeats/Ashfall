//! Server-authored GUI widget rendering (stub).

use crate::world::registry::ClientRegistry;
use eframe::egui;

pub fn render_server_gui(ui: &mut egui::Ui, _registry: &ClientRegistry) {
    ui.label("Server GUI (stub — Phase 6 full widgets pending)");
}
