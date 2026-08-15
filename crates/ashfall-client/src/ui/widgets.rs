//! Server-authored GUI — window/widget state, packet application, rendering.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use eframe::egui;
use std::collections::HashMap;

/// Widget kinds — mirrors the wire protocol's New packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiWidgetKind {
    Window,
    Button,
    Text,
    Edit,
    Checkbox,
    RadioButton,
    List,
    ListItem,
}

/// One server-authored widget.
#[derive(Debug, Clone)]
pub struct GuiWindow {
    pub id: NetworkID,
    pub parent: Option<NetworkID>,
    pub label: String,
    pub pos: [f32; 4],
    pub size: [f32; 4],
    pub locked: bool,
    pub visible: bool,
    pub text: String,
    pub kind: GuiWidgetKind,
    pub max_len: u32,
    pub validation: String,
    pub selected: bool,
    pub group: u32,
    pub multiselect: bool,
    pub items: Vec<NetworkID>,
}

impl GuiWindow {
    fn new(id: NetworkID, parent: Option<NetworkID>, label: String, kind: GuiWidgetKind) -> Self {
        GuiWindow {
            id,
            parent,
            label,
            pos: [0.0; 4],
            size: [200.0, 100.0, 0.0, 0.0],
            locked: false,
            visible: true,
            text: String::new(),
            kind,
            max_len: 0,
            validation: String::new(),
            selected: false,
            group: 0,
            multiselect: false,
            items: Vec::new(),
        }
    }
}

/// Server-authored GUI state, fed by window packets and rendered by egui.
#[derive(Default)]
pub struct GuiState {
    pub widgets: HashMap<NetworkID, GuiWindow>,
    /// Clicks not yet sent back to the server (drained by the network loop).
    pub pending_clicks: Vec<NetworkID>,
    pub window_mode: bool,
}

impl GuiState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one window/GUI packet to the state. Returns true if handled.
    pub fn apply_packet(&mut self, packet: &Packet) -> bool {
        use Packet::*;
        match packet {
            WindowNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
            } => {
                let mut w =
                    GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::Window);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                self.widgets.insert(*id, w);
            }
            ButtonNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
            } => {
                let mut w =
                    GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::Button);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                self.widgets.insert(*id, w);
            }
            TextNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
            } => {
                let mut w = GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::Text);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                self.widgets.insert(*id, w);
            }
            EditNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
                max_len,
                validation,
            } => {
                let mut w = GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::Edit);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                w.max_len = *max_len;
                w.validation = validation.clone();
                self.widgets.insert(*id, w);
            }
            CheckboxNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
                selected,
            } => {
                let mut w =
                    GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::Checkbox);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                w.selected = *selected;
                self.widgets.insert(*id, w);
            }
            RadioButtonNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
                selected,
                group,
            } => {
                let mut w = GuiWindow::new(
                    *id,
                    Some(*parent),
                    label.clone(),
                    GuiWidgetKind::RadioButton,
                );
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                w.selected = *selected;
                w.group = *group;
                self.widgets.insert(*id, w);
            }
            ListNew {
                id,
                parent,
                label,
                pos,
                size,
                locked,
                visible,
                text,
                multiselect,
            } => {
                let mut w = GuiWindow::new(*id, Some(*parent), label.clone(), GuiWidgetKind::List);
                w.pos = *pos;
                w.size = *size;
                w.locked = *locked;
                w.visible = *visible;
                w.text = text.clone();
                w.multiselect = *multiselect;
                self.widgets.insert(*id, w);
            }
            ListItemNew {
                id,
                container,
                text,
                selected,
            } => {
                let mut w =
                    GuiWindow::new(*id, Some(*container), text.clone(), GuiWidgetKind::ListItem);
                w.selected = *selected;
                if let Some(list) = self.widgets.get_mut(container) {
                    list.items.push(*id);
                }
                self.widgets.insert(*id, w);
            }
            ListItemRemove { id } => {
                self.widgets.remove(id);
            }
            WindowRemove { id } => {
                self.widgets.remove(id);
            }
            UpdateWindowPos { id, pos } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.pos = *pos;
                }
            }
            UpdateWindowSize { id, size } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.size = *size;
                }
            }
            UpdateWindowVisible { id, visible } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.visible = *visible;
                }
            }
            UpdateWindowLocked { id, locked } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.locked = *locked;
                }
            }
            UpdateWindowText { id, text } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.text = text.clone();
                }
            }
            UpdateEditMaxLen { id, max_len } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.max_len = *max_len;
                }
            }
            UpdateEditValidation { id, validation } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.validation = validation.clone();
                }
            }
            UpdateCheckboxSelected { id, selected } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.selected = *selected;
                }
            }
            UpdateRadioButtonSelected { id, selected, .. } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.selected = *selected;
                }
            }
            UpdateRadioButtonGroup { id, group } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.group = *group;
                }
            }
            UpdateListMultiSelect { id, multiselect } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.multiselect = *multiselect;
                }
            }
            UpdateListItemSelected { id, selected } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.selected = *selected;
                }
            }
            UpdateListItemText { id, text } => {
                if let Some(w) = self.widgets.get_mut(id) {
                    w.text = text.clone();
                }
            }
            UpdateWindowMode { enabled } => {
                self.window_mode = *enabled;
            }
            _ => return false,
        }
        true
    }

    /// Children of a window (widgets whose parent is `id`), for rendering.
    pub fn children(&self, id: NetworkID) -> Vec<&GuiWindow> {
        self.widgets
            .values()
            .filter(|w| w.parent == Some(id))
            .collect()
    }

    /// Top-level windows (no parent / parent 0).
    pub fn top_windows(&self) -> Vec<&GuiWindow> {
        self.widgets
            .values()
            .filter(|w| w.kind == GuiWidgetKind::Window && w.parent.is_none_or(|p| p.as_u64() == 0))
            .collect()
    }
}

/// Render the server-authored GUI: each top-level window as an egui window
/// containing its child widgets. Widget clicks are queued in `pending_clicks`
/// for the network loop to send back to the server.
pub fn render_server_gui(ui: &mut egui::Ui, gui: &mut GuiState) {
    if !gui.window_mode && gui.top_windows().is_empty() {
        ui.label("No server GUI");
        return;
    }
    let windows: Vec<GuiWindow> = gui.top_windows().into_iter().cloned().collect();
    for window in windows {
        let title = if window.label.is_empty() {
            "Window"
        } else {
            &window.label
        };
        let mut open = window.visible;
        egui::Window::new(title)
            .open(&mut open)
            .default_pos([window.pos[0], window.pos[1]])
            .default_size([window.size[0], window.size[1]])
            .resizable(!window.locked)
            .show(ui.ctx(), |ui| {
                let children: Vec<GuiWindow> = gui
                    .widgets
                    .values()
                    .filter(|w| w.parent == Some(window.id))
                    .cloned()
                    .collect();
                for child in children {
                    render_widget(ui, &child, gui);
                }
            });
    }
}

fn render_widget(ui: &mut egui::Ui, w: &GuiWindow, gui: &mut GuiState) {
    if !w.visible {
        return;
    }
    let id = w.id;
    match w.kind {
        GuiWidgetKind::Button => {
            if ui.button(&w.label).clicked() {
                gui.pending_clicks.push(id);
            }
        }
        GuiWidgetKind::Text => {
            let text = if w.text.is_empty() { &w.label } else { &w.text };
            ui.label(text);
        }
        GuiWidgetKind::Edit => {
            ui.text_edit_singleline(&mut w.text.clone());
            gui.pending_clicks.push(id); // ponytail: report focus/edit for now
        }
        GuiWidgetKind::Checkbox => {
            if ui.checkbox(&mut w.selected.clone(), &w.label).changed() {
                gui.pending_clicks.push(id);
            }
        }
        GuiWidgetKind::RadioButton => {
            if ui.radio(w.selected, &w.label).clicked() {
                gui.pending_clicks.push(id);
            }
        }
        GuiWidgetKind::List => {
            let items: Vec<String> = w
                .items
                .iter()
                .filter_map(|i| gui.widgets.get(i))
                .map(|i| i.text.clone())
                .collect();
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for item in &items {
                        let _ = ui.selectable_label(false, item);
                    }
                });
        }
        GuiWidgetKind::ListItem => {
            ui.label(&w.text);
        }
        GuiWidgetKind::Window => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(n: u64) -> NetworkID {
        NetworkID::new(n)
    }

    #[test]
    fn test_window_lifecycle() {
        let mut gui = GuiState::new();
        let id = nid(1);
        assert!(gui.apply_packet(&Packet::WindowNew {
            id,
            parent: nid(0),
            label: "Test".into(),
            pos: [10.0, 20.0, 0.0, 0.0],
            size: [300.0, 200.0, 0.0, 0.0],
            locked: true,
            visible: true,
            text: String::new(),
        }));
        let w = gui.widgets.get(&id).expect("window created");
        assert_eq!(w.kind, GuiWidgetKind::Window);
        assert_eq!(w.label, "Test");
        assert!(w.locked);
        assert!(gui.apply_packet(&Packet::UpdateWindowPos {
            id,
            pos: [1.0, 2.0, 3.0, 4.0]
        }));
        assert_eq!(gui.widgets.get(&id).unwrap().pos, [1.0, 2.0, 3.0, 4.0]);
        assert!(gui.apply_packet(&Packet::UpdateWindowText {
            id,
            text: "hi".into()
        }));
        assert_eq!(gui.widgets.get(&id).unwrap().text, "hi");
        assert!(gui.apply_packet(&Packet::UpdateWindowVisible { id, visible: false }));
        assert!(!gui.widgets.get(&id).unwrap().visible);
        assert!(gui.apply_packet(&Packet::WindowRemove { id }));
        assert!(!gui.widgets.contains_key(&id), "window removed");
    }

    #[test]
    fn test_widget_creation_and_updates() {
        let mut gui = GuiState::new();
        let win = nid(100);
        let btn = nid(101);
        let edit = nid(102);
        let chk = nid(103);
        let list = nid(104);
        let item = nid(105);
        let new_win = |id: NetworkID, label: &str| Packet::WindowNew {
            id,
            parent: nid(0),
            label: label.into(),
            pos: [0.0; 4],
            size: [0.0; 4],
            locked: false,
            visible: true,
            text: String::new(),
        };
        assert!(gui.apply_packet(&new_win(win, "W")));
        assert!(gui.apply_packet(&Packet::ButtonNew {
            id: btn,
            parent: win,
            label: "Go".into(),
            pos: [0.0; 4],
            size: [0.0; 4],
            locked: false,
            visible: true,
            text: String::new(),
        }));
        assert!(gui.apply_packet(&Packet::EditNew {
            id: edit,
            parent: win,
            label: "Name".into(),
            pos: [0.0; 4],
            size: [0.0; 4],
            locked: false,
            visible: true,
            text: String::new(),
            max_len: 16,
            validation: "alpha".into(),
        }));
        assert!(gui.apply_packet(&Packet::CheckboxNew {
            id: chk,
            parent: win,
            label: "On".into(),
            pos: [0.0; 4],
            size: [0.0; 4],
            locked: false,
            visible: true,
            text: String::new(),
            selected: true,
        }));
        assert!(gui.apply_packet(&Packet::ListNew {
            id: list,
            parent: win,
            label: "L".into(),
            pos: [0.0; 4],
            size: [0.0; 4],
            locked: false,
            visible: true,
            text: String::new(),
            multiselect: true,
        }));
        assert!(gui.apply_packet(&Packet::ListItemNew {
            id: item,
            container: list,
            text: "entry".into(),
            selected: false,
        }));

        assert_eq!(gui.widgets.get(&btn).unwrap().kind, GuiWidgetKind::Button);
        assert_eq!(gui.widgets.get(&edit).unwrap().max_len, 16);
        assert!(gui.widgets.get(&chk).unwrap().selected);
        assert!(gui.widgets.get(&list).unwrap().multiselect);
        assert!(gui.widgets.get(&list).unwrap().items.contains(&item));
        assert_eq!(
            gui.children(win).len(),
            4,
            "button/edit/checkbox/list are window children"
        );
        assert!(
            !gui.children(list).is_empty(),
            "list item parented to the list"
        );

        assert!(gui.apply_packet(&Packet::UpdateCheckboxSelected {
            id: chk,
            selected: false
        }));
        assert!(!gui.widgets.get(&chk).unwrap().selected);
        assert!(gui.apply_packet(&Packet::UpdateListItemSelected {
            id: item,
            selected: true
        }));
        assert!(gui.widgets.get(&item).unwrap().selected);
        assert!(gui.apply_packet(&Packet::UpdateListMultiSelect {
            id: list,
            multiselect: false
        }));
        assert!(!gui.widgets.get(&list).unwrap().multiselect);
        assert!(gui.apply_packet(&Packet::UpdateEditMaxLen {
            id: edit,
            max_len: 32
        }));
        assert_eq!(gui.widgets.get(&edit).unwrap().max_len, 32);
    }

    #[test]
    fn test_window_mode_packet() {
        let mut gui = GuiState::new();
        assert!(gui.apply_packet(&Packet::UpdateWindowMode { enabled: true }));
        assert!(gui.window_mode);
    }

    #[test]
    fn test_unrelated_packets_not_handled() {
        let mut gui = GuiState::new();
        assert!(!gui.apply_packet(&Packet::GameChat {
            message: "x".into()
        }));
        assert!(!gui.apply_packet(&Packet::GameLoad));
    }
}
