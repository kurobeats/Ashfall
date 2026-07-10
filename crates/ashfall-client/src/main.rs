mod config;
mod dispatch;
mod game;
mod ipc;
mod network;
mod ui;
mod world;

use config::ClientConfig;
use game::{ClientState, Game};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct AppState {
    game: Arc<Mutex<Game>>,
    chat_input: String,
    status: String,
    connect_addr: String,
}

impl AppState {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        AppState {
            game: Arc::new(Mutex::new(Game::new(ClientConfig::default()))),
            chat_input: String::new(),
            status: "Disconnected".into(),
            connect_addr: "127.0.0.1:1770".into(),
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        {
            if let Ok(mut game) = self.game.lock() {
                if game.state != ClientState::Disconnected {
                    let rt = tokio::runtime::Handle::current();
                    if let Ok(packets) = rt.block_on(game.poll()) {
                        for pkt in packets { game.handle_packet(pkt); }
                    }
                }
            }
        }

        ctx.request_repaint_after(Duration::from_millis(33));

        let (obj_count, state, chat_msgs, local_id) = {
            let game = self.game.lock().unwrap();
            (game.registry.object_count(), game.state, game.chat_messages.clone(), game.local_player_id)
        };

        self.status = match state {
            ClientState::Disconnected => "Disconnected".into(),
            ClientState::Connecting => "Connecting...".into(),
            ClientState::Authenticating => "Authenticating...".into(),
            ClientState::Loading => format!("Loading... ({} objects)", obj_count),
            ClientState::InGame => format!("In Game ({} objects)", obj_count),
        };

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Ashfall Client");
                ui.separator();
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Server:");
                    ui.add(egui::TextEdit::singleline(&mut self.connect_addr).desired_width(120.0));
                    if ui.button("Connect").clicked() {
                        let addr: std::net::SocketAddr = self.connect_addr.parse().unwrap_or_else(|_| "127.0.0.1:1770".parse().unwrap());
                        let game = self.game.clone();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                let mut g = game.lock().unwrap();
                                let _ = g.connect(addr).await;
                                let _ = g.authenticate().await;
                            });
                        });
                    }
                    if matches!(state, ClientState::InGame | ClientState::Loading) {
                        if ui.button("Disconnect").clicked() {
                            let mut game = self.game.lock().unwrap();
                            game.chat_messages.push(("System".into(), "Disconnected".into()));
                            game.state = ClientState::Disconnected;
                            game.network = None;
                        }
                    }
                });
            });
        });

        egui::SidePanel::left("left_panel").resizable(true).default_width(200.0).show(ctx, |ui| {
            ui.heading("World");
            ui.label(format!("Objects: {obj_count}"));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false; 2]).stick_to_bottom(true).show(ui, |ui| {
                for (sender, msg) in &chat_msgs {
                    ui.label(format!("{sender}: {msg}"));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let resp = ui.text_edit_singleline(&mut self.chat_input);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let msg = self.chat_input.trim().to_string();
                    if !msg.is_empty() {
                        let mut game = self.game.lock().unwrap();
                        let rt = tokio::runtime::Handle::current();
                        let _ = rt.block_on(game.send_chat(msg));
                    }
                    self.chat_input.clear();
                    resp.request_focus();
                }
                if ui.button("Send").clicked() {
                    let msg = self.chat_input.trim().to_string();
                    if !msg.is_empty() {
                        let mut game = self.game.lock().unwrap();
                        let rt = tokio::runtime::Handle::current();
                        let _ = rt.block_on(game.send_chat(msg));
                        self.chat_input.clear();
                    }
                }
            });
        });
    }
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]).with_title("Ashfall — Fallout Multiplayer"),
        ..Default::default()
    };
    eframe::run_native("Ashfall Client", options, Box::new(|cc| Ok(Box::new(AppState::new(cc)))))
}
