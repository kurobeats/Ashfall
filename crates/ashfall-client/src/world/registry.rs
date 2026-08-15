//! Lightweight client-side object cache.
//!
//! Updated by server packets, read by render/UI.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::string_cache::StringTable;
use std::collections::{HashMap, HashSet};

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
        ref_id: u32,
        pos: [f32; 3],
        angle: [f32; 3],
        dead: bool,
        health: f32,
        alerted: bool,
        sneaking: bool,
        moving: u8,
        weapon: u8,
    },
    Player {
        ref_id: u32,
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
    /// String dictionary — the server assigns ids, `Inline` payloads teach
    /// the client the mapping, `Id` references resolve against it.
    pub string_table: StringTable,
    /// Actors this client simulates (server-granted, STR OwnershipTransfer).
    pub owned_actors: HashSet<NetworkID>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        ClientRegistry {
            objects: HashMap::new(),
            cell_objects: HashMap::new(),
            weather: 0,
            globals: HashMap::new(),
            interp: HashMap::new(),
            string_table: StringTable::new(),
            owned_actors: HashSet::new(),
        }
    }

    /// Apply a server packet to update the local cache.
    pub fn apply_packet(&mut self, packet: &Packet) {
        match packet {
            Packet::ObjectNew {
                id, ref_id, name, net_pos, angle, scale, cell, enabled, ..
            } => {
                let name = name.resolve(&mut self.string_table);
                self.interp
                    .entry(*id)
                    .or_default()
                    .push_now(*net_pos);
                self.objects.insert(
                    *id,
                    ClientObject::Object {
                        ref_id: *ref_id, base_id: 0, name,
                        pos: *net_pos, angle: *angle, scale: *scale, cell: *cell, enabled: *enabled,
                    },
                );
            }
            Packet::UpdateName { id, name } => {
                let name = name.resolve(&mut self.string_table);
                if let Some(ClientObject::Object { name: n, .. }) = self.objects.get_mut(id) {
                    *n = name;
                }
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
            Packet::ActorNew { id, ref_id, values, dead, .. } => {
                let health = values.get(&0x14).copied().unwrap_or(100.0);
                self.objects.insert(*id, ClientObject::Actor {
                    ref_id: *ref_id, pos: [0.0; 3], angle: [0.0; 3], dead: *dead, health, alerted: false, sneaking: false,
                    moving: 0, weapon: 0,
                });
            }
            Packet::UpdateActorState { id, alerted, sneaking, moving, weapon, .. } => {
                if let Some(ClientObject::Actor { alerted: a, sneaking: s, moving: m, weapon: w, .. }) = self.objects.get_mut(id) {
                    *a = *alerted; *s = *sneaking; *m = *moving; *w = *weapon;
                }
            }
            Packet::ActorStateDelta { id, moving, weapon, alerted, sneaking, .. } => {
                if let Some(ClientObject::Actor { moving: m, weapon: w, alerted: a, sneaking: s, .. }) = self.objects.get_mut(id) {
                    if let Some(v) = moving { *m = *v; }
                    if let Some(v) = weapon { *w = *v; }
                    if let Some(v) = alerted { *a = *v; }
                    if let Some(v) = sneaking { *s = *v; }
                }
            }
            Packet::OwnershipGranted { id } => {
                self.owned_actors.insert(*id);
            }
            Packet::OwnershipReleased { id } => {
                self.owned_actors.remove(id);
            }
            Packet::UpdateActorValue { id, index, value, .. } => {
                if let Some(ClientObject::Actor { health, .. }) = self.objects.get_mut(id) {
                    if *index == 0x14 { *health = *value; }
                }
            }
            Packet::UpdateActorDead { id, dead, .. } => {
                if let Some(ClientObject::Actor { dead: d, .. }) = self.objects.get_mut(id) { *d = *dead; }
            }
            Packet::PlayerNew { id, ref_id, .. } => {
                self.objects.insert(*id, ClientObject::Player {
                    ref_id: *ref_id, name: format!("Player_{id}"), pos: [0.0; 3], angle: [0.0; 3], health: 100.0,
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

    /// Game ref id for a server entity (captured from its New packet), if
    /// known — used to address engine commands for remote entities.
    pub fn ref_of(&self, id: NetworkID) -> Option<u32> {
        match self.objects.get(&id) {
            Some(ClientObject::Object { ref_id, .. })
            | Some(ClientObject::Actor { ref_id, .. })
            | Some(ClientObject::Player { ref_id, .. }) => {
                if *ref_id != 0 { Some(*ref_id) } else { None }
            }
            _ => None,
        }
    }

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
            .or_default()
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

    #[test]
    fn test_object_name_string_cache_resolved() {
        use ashfall_core::string_cache::CachedString;
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(11);

        // First sight: Inline { id, value } — client learns the mapping.
        reg.apply_packet(&Packet::ObjectNew {
            id, ref_id: 0x100, base_id: 0x200,
            name: CachedString::Inline { id: 3, value: "Vault101Door".into() },
            game_pos: [0.0, 0.0, 0.0], net_pos: [0.0, 0.0, 0.0],
            angle: [0.0; 3], scale: 1.0, cell: 1, enabled: true, lock: 0, owner: 0,
        });
        match reg.objects.get(&id).unwrap() {
            ClientObject::Object { name, .. } => assert_eq!(name, "Vault101Door"),
            _ => panic!("expected object"),
        }

        // Repeat: Id only — resolves against the learned table.
        reg.apply_packet(&Packet::UpdateName { id, name: CachedString::Id(3) });
        match reg.objects.get(&id).unwrap() {
            ClientObject::Object { name, .. } => assert_eq!(name, "Vault101Door", "Id resolved via table"),
            _ => panic!("expected object"),
        }
        assert_eq!(reg.string_table.lookup(3), Some("Vault101Door"));
    }

    #[test]
    fn test_actor_state_delta_merges_present_fields() {
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(12);
        reg.apply_packet(&Packet::ActorNew {
            id, ref_id: 0x500, base_id: 0x1234, values: Default::default(),
            base_values: Default::default(), race: 0, age: 0, idle: 0, moving: 0,
            moving_xy: 0, weapon: 0, female: false, alerted: false, sneaking: false,
            dead: false, death_limbs: 0, death_cause: 0, scale: 1.0,
        });

        // Delta touches only weapon + sneaking; moving/alerted must survive.
        reg.apply_packet(&Packet::ActorStateDelta {
            id, idle: None, moving: None, moving_xy: None,
            weapon: Some(0x2A), alerted: None, sneaking: Some(true), firing: None,
        });
        match reg.objects.get(&id).unwrap() {
            ClientObject::Actor { weapon, sneaking, moving, alerted, .. } => {
                assert_eq!(*weapon, 0x2A);
                assert!(*sneaking);
                assert_eq!(*moving, 0, "untouched field kept");
                assert!(!*alerted, "untouched field kept");
            }
            _ => panic!("expected actor"),
        }
    }

    #[test]
    fn test_ownership_sets_tracked() {
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(13);
        reg.apply_packet(&Packet::OwnershipGranted { id });
        assert!(reg.owned_actors.contains(&id));
        reg.apply_packet(&Packet::OwnershipReleased { id });
        assert!(!reg.owned_actors.contains(&id));
    }
}


    #[test]
    fn test_actor_health_syncs_from_actor_value_events() {
        let mut reg = ClientRegistry::new();
        let id = NetworkID::new(11);
        reg.apply_packet(&Packet::ActorNew {
            id,
            ref_id: 0x100,
            base_id: 0x200,
            values: Default::default(),
            base_values: Default::default(),
            race: 0,
            age: 0,
            idle: 0,
            moving: 0,
            moving_xy: 0,
            weapon: 0,
            female: false,
            alerted: false,
            sneaking: false,
            dead: false,
            death_limbs: 0,
            death_cause: 0,
            scale: 1.0,
        });
        // object type Actor: health defaults to 100 via the values map.
        assert!(matches!(reg.objects.get(&id), Some(ClientObject::Actor { health, .. }) if *health == 100.0));
        // server relays a remote actor-value update (index 0x14 = health).
        reg.apply_packet(&Packet::UpdateActorValue { id, base: false, index: 0x14, value: 37.5 });
        match reg.objects.get(&id).unwrap() {
            ClientObject::Actor { health, .. } => assert_eq!(*health, 37.5),
            _ => panic!("expected actor"),
        }
        // a non-health index must not touch health.
        reg.apply_packet(&Packet::UpdateActorValue { id, base: false, index: 0x29, value: 10.0 });
        match reg.objects.get(&id).unwrap() {
            ClientObject::Actor { health, .. } => assert_eq!(*health, 37.5),
            _ => panic!("expected actor"),
        }
    }
