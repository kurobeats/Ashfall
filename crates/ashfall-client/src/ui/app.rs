use crate::game::Game;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct AshfallApp {
    game: Arc<Mutex<Game>>,
    connecting: bool,
    connect_addr: String,
    connect_port: u16,
}

impl AshfallApp {
    pub fn new(game: Arc<Mutex<Game>>) -> Self {
        AshfallApp {
            game,
            connecting: false,
            connect_addr: String::from("127.0.0.1"),
            connect_port: 1770,
        }
    }
}

impl eframe::App for AshfallApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(game) = self.game.lock() {
            let connected =
                matches!(game.state, crate::game::ClientState::InGame);

            // Top bar
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(if connected {
                        "🟢 Connected"
                    } else {
                        "🔴 Disconnected"
                    });
                    ui.separator();
                    ui.label(format!("Player: {}", game.config.name));
                });
            });

            // Main content
            if !connected {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("🌍 Server Browser");
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Address:");
                        ui.text_edit_singleline(&mut self.connect_addr);
                        ui.label("Port:");
                        ui.add(
                            egui::DragValue::new(&mut self.connect_port).range(1..=65535),
                        );
                        if ui.button("Connect").clicked() {
                            self.connecting = true;
                        }
                    });
                });
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("🌍 Wasteland — Connected to server");
                    ui.separator();
                    ui.label(format!(
                        "Objects tracked: {}",
                        game.registry.object_count()
                    ));
                    if let Some(ref id) = game.local_player_id {
                        ui.label(format!("Player ID: {id}"));
                    }
                });

                // Chat
                egui::TopBottomPanel::bottom("chat_panel").show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for (sender, msg) in &game.chat_messages {
                                ui.label(format!("{sender}: {msg}"));
                            }
                        });
                });
            }
        }

        // Handle pending connection
        if self.connecting {
            self.connecting = false;
            let addr_str = format!("{}:{}", self.connect_addr, self.connect_port);
            if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
                let game_arc = self.game.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let mut g = game_arc.lock().unwrap();
                        let _ = g.connect(addr).await;
                        let _ = g.authenticate().await;
                    });
                });
            }
        }
    }
}
