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
        if let Ok(mut game) = self.game.lock() {
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
                    if let Some(t) = game.game_time {
                        ui.separator();
                        ui.label(format!(
                            "🕐 {}-{:02}-{:02} {:02}:00 ({:.0}x)",
                            t.year, t.month, t.day, t.hour, t.time_scale
                        ));
                    }
                    if game.pvp_enabled {
                        ui.separator();
                        ui.label("⚔ PvP");
                    }
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
                    ui.separator();
                    // Server-authored GUI (windows/buttons/edits/lists)
                    crate::ui::widgets::render_server_gui(ui, &mut game.gui);
                    ui.separator();
                    // Top-down world view (X right, Z up) centered on the
                    // local player, remote objects as dots.
                    let view_size = ui.available_size();
                    let (rect, _) = ui.allocate_exact_size(view_size, egui::Sense::hover());
                    crate::ui::world_view::draw_world(
                        &ui.painter_at(rect),
                        rect,
                        &game.registry,
                        game.local_player_id,
                    );
                    ui.separator();
                    // Remote objects with interpolated positions
                    ui.collapsing(format!("Objects ({})", game.registry.object_count()), |ui| {
                        let ids: Vec<ashfall_core::id::NetworkID> =
                            game.registry.get_objects().map(|(id, _)| *id).collect();
                        for id in ids {
                            if let Some(pos) = game.registry.interpolated_pos(id) {
                                let p = format!("({:.1}, {:.1}, {:.1})", pos[0], pos[1], pos[2]);
                                ui.label(format!("{id} @ {p}"));
                            }
                        }
                    });
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
