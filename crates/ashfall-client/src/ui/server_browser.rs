//! Server browser — master server query and display.

use ashfall_core::protocol::Packet;
use eframe::egui;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::time::Duration;

/// Displayed server entry.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub addr: SocketAddr,
    pub map: String,
    pub players: u32,
    pub max_players: u32,
    pub game_type: String,
}

/// Server browser panel — queries master, lists servers, handles join.
pub struct ServerBrowser {
    master_addr: String,
    servers: Vec<ServerInfo>,
    refreshing: bool,
    query_rx: Option<mpsc::Receiver<Vec<ServerInfo>>>,
    pub selected_addr: Option<SocketAddr>,
    pub direct_addr: String,
}

impl ServerBrowser {
    pub fn new(master_addr: String) -> Self {
        ServerBrowser {
            master_addr,
            servers: Vec::new(),
            refreshing: false,
            query_rx: None,
            selected_addr: None,
            direct_addr: "127.0.0.1:1770".into(),
        }
    }

    /// Refresh server list from master.
    pub fn refresh(&mut self) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;

        let master_addr = self.master_addr.clone();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = query_master_sync(&master_addr);
            let _ = tx.send(result);
        });

        self.query_rx = Some(rx);
    }

    /// Poll for query results (call from egui update).
    fn poll_results(&mut self) {
        if let Some(ref rx) = self.query_rx {
            if let Ok(servers) = rx.try_recv() {
                self.servers = servers;
                self.refreshing = false;
                self.query_rx = None;
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> &mut Self {
        self.poll_results();

        ui.heading("Server Browser");

        // Direct connect input
        ui.horizontal(|ui| {
            ui.label("Direct connect:");
            ui.text_edit_singleline(&mut self.direct_addr);
            if ui.button("Connect").clicked() {
                if let Ok(addr) = self.direct_addr.parse::<SocketAddr>() {
                    self.selected_addr = Some(addr);
                }
            }
        });

        ui.separator();

        // Refresh button
        let refresh_btn = egui::Button::new(if self.refreshing { "Searching..." } else { "Refresh" });
        if ui.add_enabled(!self.refreshing, refresh_btn).clicked() {
            self.refresh();
        }

        // Server list
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            if self.servers.is_empty() {
                ui.label(if self.refreshing {
                    "Querying master server..."
                } else {
                    "No servers found. Click Refresh to search."
                });
            }

            let mut to_select: Option<SocketAddr> = None;
            for server in &self.servers {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&server.name);
                            ui.label(format!("{} — {}/{} players", server.map, server.players, server.max_players));
                            ui.label(format!("{} ({})", server.addr, server.game_type));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Join").clicked() {
                                to_select = Some(server.addr);
                            }
                        });
                    });
                });
            }

            if let Some(addr) = to_select {
                self.selected_addr = Some(addr);
            }
        });

        self
    }
}

/// Synchronous master query (runs on background thread).
fn query_master_sync(master_addr: &str) -> Vec<ServerInfo> {
    use std::net::UdpSocket;

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));

    // Send MasterQuery
    let packet = Packet::MasterQuery;
    let payload = match postcard::to_stdvec(&packet) {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    let mut buf = Vec::with_capacity(3 + payload.len());
    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    buf.push(0u8);
    buf.extend_from_slice(&payload);

    if socket.send_to(&buf, master_addr).is_err() {
        return vec![];
    }

    // Collect responses for up to 2 seconds
    let mut servers = Vec::new();
    let mut recv_buf = [0u8; 2048];
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(2) {
        match socket.recv_from(&mut recv_buf) {
            Ok((len, src)) => {
                if let Some(packet) = decode_response(&recv_buf[..len]) {
                    if let Packet::MasterAnnounce { name, map, players, max_players, game_type, .. } = packet {
                        servers.push(ServerInfo {
                            name,
                            addr: src,
                            map,
                            players,
                            max_players,
                            game_type,
                        });
                    }
                }
            }
            Err(_) => break, // timeout
        }
    }

    // Dedup by addr (keep first)
    servers.sort_by(|a, b| a.addr.cmp(&b.addr));
    servers.dedup_by(|a, b| a.addr == b.addr);

    servers
}

fn decode_response(data: &[u8]) -> Option<Packet> {
    if data.len() < 3 {
        return None;
    }
    let _length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let _channel = data[2];
    let payload = &data[3..];
    postcard::from_bytes(payload).ok()
}
