//! egui painter for the top-down world view.

use crate::world::registry::{ClientObject, ClientRegistry};
use crate::world::view::WorldView;
use ashfall_core::id::NetworkID;
use eframe::egui::{self, Color32, Rect, Stroke};

/// World units per pixel at default zoom (1:1 — a 100-unit cell is 100px).
const DEFAULT_SCALE: f32 = 1.0;

/// Draw the top-down world view into `rect` with `painter`.
///
/// Center: the local player's interpolated position (or origin pre-spawn).
/// Remote objects are dots colored by type: player green, actor red,
/// object gray, item blue.
pub fn draw_world(
    painter: &egui::Painter,
    rect: Rect,
    registry: &ClientRegistry,
    local_player: Option<NetworkID>,
) {
    let size = [rect.width(), rect.height()];
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return;
    }

    // Center on the local player, fall back to the origin.
    let center = local_player
        .and_then(|id| registry.interpolated_pos(id))
        .map(|p| [p[0], p[2]])
        .unwrap_or([0.0, 0.0]);
    let view = WorldView::centered_on(center, DEFAULT_SCALE, size);

    // Background + border.
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 22, 26));
    painter.rect_stroke(
        rect,
        0.0_f32,
        Stroke::new(1.0_f32, Color32::from_rgb(60, 70, 80)),
    );

    // Origin cross (in case the local player is elsewhere).
    let o = view.world_to_screen(0.0, 0.0);
    let o = egui::pos2(rect.left() + o[0], rect.top() + o[1]);
    painter.line_segment(
        [
            o + egui::vec2(-4.0_f32, 0.0_f32),
            o + egui::vec2(4.0_f32, 0.0_f32),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(70, 80, 90)),
    );
    painter.line_segment(
        [
            o + egui::vec2(0.0_f32, -4.0_f32),
            o + egui::vec2(0.0_f32, 4.0_f32),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(70, 80, 90)),
    );

    // Remote objects.
    let ids: Vec<NetworkID> = registry.get_objects().map(|(id, _)| *id).collect();
    for id in ids {
        let Some(pos) = registry.interpolated_pos(id) else {
            continue;
        };
        let screen = view.world_to_screen(pos[0], pos[2]);
        let p = egui::pos2(rect.left() + screen[0], rect.top() + screen[1]);
        if !rect.expand(8.0).contains(p) {
            continue; // off-viewport
        }
        // (color, health for bar, dead flag, name label)
        let (color, health, dead, name) = match registry.get(id) {
            Some(ClientObject::Player { name, health, .. }) => (
                Color32::from_rgb(90, 200, 120),
                Some(*health),
                false,
                Some(name.clone()),
            ),
            Some(ClientObject::Actor { health, dead, .. }) => {
                (Color32::from_rgb(220, 90, 90), Some(*health), *dead, None)
            }
            Some(ClientObject::Item { .. }) => (Color32::from_rgb(90, 150, 220), None, false, None),
            _ => (Color32::from_rgb(150, 150, 150), None, false, None),
        };
        let dot = if dead {
            Color32::from_rgb(90, 90, 90)
        } else {
            color
        };
        painter.circle_filled(p, 3.0, dot);
        // Health bar for actors/players (ratio against the default 100 max).
        if let Some(h) = health {
            let ratio = (h / 100.0).clamp(0.0, 1.0);
            let bar_w = 16.0_f32;
            let bar_h = 2.0_f32;
            let bar_origin = p + egui::vec2(-bar_w / 2.0, 5.0);
            let bg = egui::Rect::from_min_size(bar_origin, egui::vec2(bar_w, bar_h));
            painter.rect_filled(bg, 1.0, Color32::from_rgb(40, 40, 40));
            if !dead {
                let fill = (bar_w * ratio).max(0.0);
                let col = if ratio > 0.5 {
                    Color32::from_rgb(80, 200, 80)
                } else if ratio > 0.25 {
                    Color32::from_rgb(220, 200, 60)
                } else {
                    Color32::from_rgb(220, 60, 60)
                };
                painter.rect_filled(
                    egui::Rect::from_min_size(bar_origin, egui::vec2(fill, bar_h)),
                    1.0,
                    col,
                );
            }
        }
        // Player name label below the bar.
        if let Some(n) = name {
            painter.text(
                p + egui::vec2(0.0, 9.0),
                egui::Align2::CENTER_TOP,
                n,
                egui::FontId::proportional(9.0),
                Color32::from_rgb(200, 200, 210),
            );
        }
    }
}
