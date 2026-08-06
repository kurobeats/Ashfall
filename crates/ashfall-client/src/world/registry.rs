//! Lightweight client-side object cache.
//!
//! Updated by server packets, read by render/UI.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::collections::HashMap;

/// A client-side object — owned data, no locks.
/// ponytail: several fields are written by apply_packet but not yet read
/// (no renderer until engine IPC lands).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ClientObject {
    Object {
        ref_id: u32,
        base_id: u32,
        name: String,
        pos: [f32; 3],
        angle: [f32; 3],
        scale: f32,
        cell: u32,
        enabled: bool,
    },
    Item {
        base_id: u32,
        cond: f32,
        count: u32,
        equipped: bool,
    },
    Actor {
        pos: [f32; 3],
        angle: [f32; 3],
        dead: bool,
        health: f32,
        alerted: bool,
        sneaking: bool,
    },
    Player {
        name: String,
        pos: [f32; 3],
        angle: [f32; 3],
        health: f32,
    },
}

/// Client-side object registry.
/// Client-side object cache — populated by server packets, read by the
/// renderer.
pub struct ClientRegistry {
    pub objects: HashMap<NetworkID, ClientObject>,
    /// Written by packets; read once per-cell rendering lands.
    #[allow(dead_code)]
    pub cell_objects: HashMap<u32, Vec<NetworkID>>,
    #[allow(dead_code)]
    pub weather: u32,
    #[allow(dead_code)]
    pub globals: HashMap<u32, i32>,
    /// Per-entity render-behind interpolation buffers.
    interp: HashMap<NetworkID, crate::world::state::InterpBuffer>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        ClientRegistry {
            objects: HashMap::new(),
            cell_objects: HashMap::new(),
            weather: 0,
            globals: HashMap::new(),
            interp: HashMap::new(),
        }
    }

    /// Apply a server packet to update the local cache.
    pub fn apply_packet(&mut self, packet: &Packet) {
        match packet {
            Packet::ObjectNew {
                id, name, net_pos, angle, scale, cell, enabled, ..
            } => {
                self.interp
                    .entry(*id)
                    .or_insert_with(crate::world::state::InterpBuffer::new)
                    .push_now(*net_pos);
                self.objects.insert(
                    *id,
                    ClientObject::Object {
                        ref_id: 0, base_id: 0, name: name.clone(),
                        pos: *net_pos, angle: *angle, scale: *scale, cell: *cell, enabled: *enabled,
                    },
                );
            }
            Packet::UpdatePos { id, pos } => { self.update_pos(*id, *pos); }
            Packet::ObjectRemove { id, .. } => { self.objects.remove(id); self.interp.remove(id); }
            Packet::ItemNew { id, base_id, count, condition, equipped, .. } => {
                self.objects.insert(*id, ClientObject::Item {
                    base_id: *base_id, cond: *condition, count: *count, equipped: *equipped,
                });
            }
            Packet::UpdateItemCount { id, count, .. } => {
                if let Some(ClientObject::Item { count: c, .. }) = self.objects.get_mut(id) { *c = *count; }
            }
            Packet::UpdateItemCondition { id, condition, .. } => {
                if let Some(ClientObject::Item { cond: c, .. }) = self.objects.get_mut(id) { *c = *condition; }
            }
            Packet::UpdateItemEquipped { id, equipped, .. } => {
                if let Some(ClientObject::Item { equipped: e, .. }) = self.objects.get_mut(id) { *e = *equipped; }
            }
            Packet::ActorNew { id, values, dead, .. } => {
                let health = values.get(&0x14).copied().unwrap_or(100.0);
                self.objects.insert(*id, ClientObject::Actor {
                    pos: [0.0; 3], angle: [0.0; 3], dead: *dead, health, alerted: false, sneaking: false,
                });
            }
            Packet::UpdateActorState { id, alerted, sneaking, .. } => {
                if let Some(ClientObject::Actor { alerted: a, sneaking: s, .. }) = self.objects.get_mut(id) {
                    *a = *alerted; *s = *sneaking;
                }
            }
            Packet::UpdateActorValue { id, index, value, .. } => {
                if let Some(ClientObject::Actor { health, .. }) = self.objects.get_mut(id) {
                    if *index == 0x14 { *health = *value; }
                }
            }
            Packet::UpdateActorDead { id, dead, .. } => {
                if let Some(ClientObject::Actor { dead: d, .. }) = self.objects.get_mut(id) { *d = *dead; }
            }
            Packet::PlayerNew { id, .. } => {
                self.objects.insert(*id, ClientObject::Player {
                    name: format!("Player_{id}"), pos: [0.0; 3], angle: [0.0; 3], health: 100.0,
                });
            }
            _ => {}
        }
    }

    /// Look up a client object (used by the renderer once IPC lands).
    #[allow(dead_code)]
    pub fn get(&self, id: NetworkID) -> Option<&ClientObject> {
        self.objects.get(&id)
    }

    /// Iterate all client objects (used by the renderer once IPC lands).
    #[allow(dead_code)]
    pub fn get_objects(&self) -> impl Iterator<Item = (&NetworkID, &ClientObject)> {
        self.objects.iter()
    }

    pub fn object_count(&self) -> usize { self.objects.len() }

    /// Interpolated position of an object: render-behind buffer with
    /// extrapolation (mojave-online semantics — see `world::state`), falling
    /// back to the raw position when the object type has no position.
    pub fn interpolated_pos(&self, id: NetworkID) -> Option<[f32; 3]> {
        let pos = match self.objects.get(&id) {
            Some(ClientObject::Object { pos: p, .. })
            | Some(ClientObject::Actor { pos: p, .. })
            | Some(ClientObject::Player { pos: p, .. }) => Some(*p),
            _ => None,
        }?;
        Some(
            self.interp
                .get(&id)
                .map(crate::world::state::InterpBuffer::render_now)
                .unwrap_or(pos),
        )
    }

    fn update_pos(&mut self, id: NetworkID, pos: [f32; 3]) {
        self.interp
            .entry(id)
            .or_insert_with(crate::world::state::InterpBuffer::new)
            .push_now(pos);
        match self.objects.get_mut(&id) {
            Some(ClientObject::Object { pos: p, .. }) | Some(ClientObject::Actor { pos: p, .. }) | Some(ClientObject::Player { pos: p, .. }) => *p = pos,
            _ => {}
        }
    }
}

impl Default for ClientRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashfall_core::protocol::Packet;

    #[test]
    fn test_interpolation_wiring() {
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(7);
        // ObjectNew at [0,0,0], then UpdatePos to [100,0,0]
        reg.apply_packet(&Packet::ObjectNew {
            id, ref_id: 0x100, base_id: 0x200, name: "x".into(),
            game_pos: [0.0, 0.0, 0.0], net_pos: [0.0, 0.0, 0.0],
            angle: [0.0; 3], scale: 1.0, cell: 1, enabled: true, lock: 0, owner: 0,
        });
        reg.apply_packet(&Packet::UpdatePos { id, pos: [100.0, 0.0, 0.0] });

        // Render-behind (INTERP_DELAY=67ms): immediately after the update the
        // render time still lands on the old sample, so the interpolated
        // position trails at the previous position. Smoothness comes from the
        // buffer (unit-tested in world::state), not from zero-latency.
        let p = reg.interpolated_pos(id).unwrap();
        assert_eq!(p[0], 0.0, "render-behind trails at the old sample");
        // Raw position is the new one.
        match reg.objects.get(&id).unwrap() {
            ClientObject::Object { pos, .. } => assert_eq!(*pos, [100.0, 0.0, 0.0]),
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn test_interp_buffer_used() {
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(9);
        reg.apply_packet(&Packet::ObjectNew {
            id, ref_id: 0x100, base_id: 0x200, name: "y".into(),
            game_pos: [0.0, 0.0, 0.0], net_pos: [0.0, 0.0, 0.0],
            angle: [0.0; 3], scale: 1.0, cell: 1, enabled: true, lock: 0, owner: 0,
        });
        assert!(reg.interp.contains_key(&id), "buffer created on spawn");
        reg.apply_packet(&Packet::ObjectRemove { id, silent: true });
        assert!(!reg.interp.contains_key(&id), "buffer dropped on despawn");
    }
}
