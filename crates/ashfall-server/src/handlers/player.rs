//! Player handler — spawn, controls, cell context, console.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::session::Session;
use crate::world::cell::CellContext;
use crate::world::objects::Player;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Handle PlayerNew — create player, insert into registry, broadcast.
pub fn handle_player_new(
    registry: &Arc<ObjectRegistry>,
    packet: &Packet,
) -> Option<Packet> {
    let (id, ref_id, base_id, controls, scale) = match packet {
        Packet::PlayerNew { id, ref_id, base_id, controls, scale } => {
            (*id, *ref_id, *base_id, controls.clone(), *scale)
        }
        _ => return None,
    };

    if registry.is_deleted(id) {
        return None;
    }

    let mut player = Player::new(id, ref_id, base_id, 0);
    player.controls = controls;
    player.object.scale = scale;

    registry.insert(player);

    Some(packet.clone())
}

/// Handle UpdateControl — player control binding change.
pub fn handle_update_control(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    control: u8,
    key: u8,
) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.controls.insert(control, (key, true));
        }
    }
    Some(Packet::UpdateControl { id, control, key })
}

/// Handle UpdateContext — cell context change.
pub fn handle_update_context(
    registry: &Arc<ObjectRegistry>,
    session: &mut Session,
    id: NetworkID,
    cells: [u32; 9],
    spawn: bool,
) -> Vec<Packet> {
    let old_ctx = CellContext { cells: session.cell_context };
    let new_ctx = CellContext { cells };

    session.update_cell_context(cells);

    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.container.object.cell = cells[4];
        }
    }

    let (enter, leave) = old_ctx.diff(&new_ctx);
    let mut packets = Vec::new();

    // Send ObjectNew for objects in entered cells
    for cell in &enter {
        for obj_id in registry.get_by_cell(*cell) {
            if let Some(arc) = registry.get(obj_id) {
                let guard = arc.read();
                if let Some(obj) = guard.as_any().downcast_ref::<crate::world::objects::Object>() {
                    packets.push(obj.to_new_packet());
                }
            }
        }
    }

    // G7: Send ObjectRemove for objects exclusive to left cells
    let new_cells: std::collections::HashSet<u32> = cells.iter().copied().collect();
    for cell in &leave {
        for obj_id in registry.get_by_cell(*cell) {
            // Check if object is in any remaining context cell
            let still_visible = {
                let arc = registry.get(obj_id);
                arc.map(|a| {
                    let guard = a.read();
                    if let Some(obj) = guard.as_any().downcast_ref::<crate::world::objects::Object>() {
                        new_cells.contains(&obj.cell)
                    } else {
                        false
                    }
                }).unwrap_or(false)
            };
            if !still_visible {
                packets.push(Packet::ObjectRemove { id: obj_id, silent: true });
            }
        }
    }

    packets.push(Packet::UpdateContext { id, cells, spawn });
    packets
}

/// Handle console toggle.
pub fn handle_console(registry: &Arc<ObjectRegistry>, id: NetworkID, enabled: bool) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.console_enabled = enabled;
        }
    }
    Some(Packet::UpdateConsole { id, enabled })
}
