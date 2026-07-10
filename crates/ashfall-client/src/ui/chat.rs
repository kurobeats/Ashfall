//! Chat panel — message display + input.

use crate::game::Game;
use eframe::egui;

pub struct ChatPanel {
    pub input: String,
}

impl ChatPanel {
    pub fn new() -> Self { ChatPanel { input: String::new() } }

    pub fn show(&mut self, ui: &mut egui::Ui, game: &mut Game) {
        ui.horizontal(|ui| {
            ui.label("Chat:");
            if ui.text_edit_singleline(&mut self.input).lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                let msg = self.input.trim().to_string();
                if !msg.is_empty() {
                    game.chat_messages.push((game.config.name.clone(), msg.clone()));
                }
                self.input.clear();
            }
            if ui.button("Send").clicked() {
                let msg = self.input.trim().to_string();
                if !msg.is_empty() {
                    game.chat_messages.push((game.config.name.clone(), msg.clone()));
                    self.input.clear();
                }
            }
        });
        egui::ScrollArea::vertical().max_height(100.0).stick_to_bottom(true).show(ui, |ui| {
            for (sender, msg) in &game.chat_messages {
                ui.label(format!("{sender}: {msg}"));
            }
        });
    }
}
