//! Lightweight client-side object cache.
//!
//! Updated by server packets, read by render/UI.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::collections::HashMap;

/// A client-side object — owned data, no locks.
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
pub struct ClientRegistry {
    pub objects: HashMap<NetworkID, ClientObject>,
    pub cell_objects: HashMap<u32, Vec<NetworkID>>,
    pub weather: u32,
    pub globals: HashMap<u32, i32>,
    last_positions: HashMap<NetworkID, ([f32; 3], std::time::Instant)>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        ClientRegistry {
            objects: HashMap::new(),
            cell_objects: HashMap::new(),
            weather: 0,
            globals: HashMap::new(),
            last_positions: HashMap::new(),
        }
    }

    /// Apply a server packet to update the local cache.
    pub fn apply_packet(&mut self, packet: &Packet) {
        match packet {
            Packet::ObjectNew {
                id, name, net_pos, angle, scale, cell, enabled, ..
            } => {
                let now = std::time::Instant::now();
                if let Some(ClientObject::Object { pos, .. }) | Some(ClientObject::Actor { pos, .. }) = self.objects.get(id) {
                    self.last_positions.insert(*id, (*pos, now));
                }
                self.objects.insert(
                    *id,
                    ClientObject::Object {
                        ref_id: 0, base_id: 0, name: name.clone(),
                        pos: *net_pos, angle: *angle, scale: *scale, cell: *cell, enabled: *enabled,
                    },
                );
            }
            Packet::UpdatePos { id, pos } => { self.update_pos(*id, *pos); }
            Packet::ObjectRemove { id, .. } => { self.objects.remove(id); self.last_positions.remove(id); }
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

    pub fn get(&self, id: NetworkID) -> Option<&ClientObject> {
        self.objects.get(&id)
    }

    pub fn get_objects(&self) -> impl Iterator<Item = (&NetworkID, &ClientObject)> {
        self.objects.iter()
    }

    pub fn object_count(&self) -> usize { self.objects.len() }

    fn update_pos(&mut self, id: NetworkID, pos: [f32; 3]) {
        let old = match self.objects.get(&id) {
            Some(ClientObject::Object { pos: p, .. }) | Some(ClientObject::Actor { pos: p, .. }) | Some(ClientObject::Player { pos: p, .. }) => Some(*p),
            _ => None,
        };
        if let Some(old) = old {
            self.last_positions.insert(id, (old, std::time::Instant::now()));
        }
        match self.objects.get_mut(&id) {
            Some(ClientObject::Object { pos: p, .. }) | Some(ClientObject::Actor { pos: p, .. }) | Some(ClientObject::Player { pos: p, .. }) => *p = pos,
            _ => {}
        }
    }
}

impl Default for ClientRegistry {
    fn default() -> Self { Self::new() }
}
