//! Server-side game object data structs.
//!
//! These hold the authoritative state for all synced entities.
//! Serialized into Packet variants for network transmission.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::{GameObject, ObjectKind};
use std::any::Any;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
// Reference — base identity
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Reference {
    pub id: NetworkID,
    pub ref_id: u32,
    pub base_id: u32,
}

impl Reference {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32) -> Self {
        Reference { id, ref_id, base_id }
    }
}

impl GameObject for Reference {
    fn id(&self) -> NetworkID { self.id }
    fn kind(&self) -> ObjectKind { ObjectKind::Reference }
    fn kind_mask(&self) -> u32 { ObjectKind::Reference as u32 }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

// ═══════════════════════════════════════════════════════════════
// Object — positioned world entity
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Object {
    pub ref_data: Reference,
    pub name: String,
    pub game_pos: [f32; 3],
    pub net_pos: [f32; 3],
    pub angle: [f32; 3],
    pub scale: f32,
    pub cell: u32,
    pub enabled: bool,
    pub lock_level: u32,
    pub owner: u32,
    // Physics
    pub velocity: [f32; 3],
    pub on_ground: bool,
}

impl Object {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32, cell: u32) -> Self {
        Object {
            ref_data: Reference::new(id, ref_id, base_id),
            name: String::new(),
            game_pos: [0.0; 3],
            net_pos: [0.0; 3],
            angle: [0.0; 3],
            scale: 1.0,
            cell,
            enabled: true,
            lock_level: 0,
            owner: 0,
            velocity: [0.0; 3],
            on_ground: true,
        }
    }

    pub fn to_new_packet(&self) -> Packet {
        Packet::ObjectNew {
            id: self.id(),
            ref_id: self.ref_data.ref_id,
            base_id: self.ref_data.base_id,
            name: self.name.clone(),
            game_pos: self.game_pos,
            net_pos: self.net_pos,
            angle: self.angle,
            scale: self.scale,
            cell: self.cell,
            enabled: self.enabled,
            lock: self.lock_level,
            owner: self.owner,
        }
    }
}

impl GameObject for Object {
    fn id(&self) -> NetworkID { self.ref_data.id }
    fn kind(&self) -> ObjectKind { ObjectKind::Object }
    fn kind_mask(&self) -> u32 { ObjectKind::Object as u32 }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

// ═══════════════════════════════════════════════════════════════
// Item — inventory object
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Item {
    pub ref_data: Reference,
    pub container: NetworkID,
    pub count: u32,
    pub condition: f32,
    pub equipped: bool,
    pub silent: bool,
    pub stick: bool,
    pub scale: f32,
}

impl Item {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32, container: NetworkID) -> Self {
        Item {
            ref_data: Reference::new(id, ref_id, base_id),
            container,
            count: 1,
            condition: 1.0,
            equipped: false,
            silent: false,
            stick: false,
            scale: 1.0,
        }
    }

    pub fn to_new_packet(&self) -> Packet {
        Packet::ItemNew {
            id: self.id(),
            ref_id: self.ref_data.ref_id,
            base_id: self.ref_data.base_id,
            container: self.container,
            count: self.count,
            condition: self.condition,
            equipped: self.equipped,
            silent: self.silent,
            stick: self.stick,
            scale: self.scale,
        }
    }
}

impl GameObject for Item {
    fn id(&self) -> NetworkID { self.ref_data.id }
    fn kind(&self) -> ObjectKind { ObjectKind::Item }
    fn kind_mask(&self) -> u32 {
        ObjectKind::Object as u32 | ObjectKind::Item as u32
    }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

// ═══════════════════════════════════════════════════════════════
// Container — chest, NPC inventory
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Container {
    pub object: Object,
    pub items: Vec<NetworkID>,
}

impl Container {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32, cell: u32) -> Self {
        Container {
            object: Object::new(id, ref_id, base_id, cell),
            items: Vec::new(),
        }
    }

    pub fn to_new_packets(&self) -> Vec<Packet> {
        vec![
            Packet::ContainerNew {
                id: self.id(),
                ref_id: self.ref_data.ref_id,
                base_id: self.ref_data.base_id,
            },
            self.object.to_new_packet(),
        ]
    }
}

impl GameObject for Container {
    fn id(&self) -> NetworkID { self.object.id() }
    fn kind(&self) -> ObjectKind { ObjectKind::Container }
    fn kind_mask(&self) -> u32 {
        ObjectKind::Object as u32 | ObjectKind::ItemList as u32 | ObjectKind::Container as u32
    }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl std::ops::Deref for Container {
    type Target = Object;
    fn deref(&self) -> &Object { &self.object }
}

impl std::ops::DerefMut for Container {
    fn deref_mut(&mut self) -> &mut Object { &mut self.object }
}

// ═══════════════════════════════════════════════════════════════
// Actor — NPC/creature with stats
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Actor {
    pub container: Container,
    pub values: HashMap<u8, f32>,
    pub base_values: HashMap<u8, f32>,
    pub race: u32,
    pub age: i32,
    pub idle_anim: u32,
    pub moving_anim: u8,
    pub moving_xy: u8,
    pub weapon_anim: u8,
    pub female: bool,
    pub alerted: bool,
    pub sneaking: bool,
    pub dead: bool,
    pub death_limbs: u16,
    pub death_cause: i8,
    // NPC AI
    pub combat_target: Option<NetworkID>,
    pub ai_package: u32,
    pub ai_flags: u8,
    // Faction
    pub factions: Vec<(u32, i8)>, // (faction_id, rank)
}

impl Actor {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32, cell: u32) -> Self {
        Actor {
            container: Container::new(id, ref_id, base_id, cell),
            values: HashMap::new(),
            base_values: HashMap::new(),
            race: 0,
            age: 0,
            idle_anim: 0,
            moving_anim: 0,
            moving_xy: 0,
            weapon_anim: 0,
            female: false,
            alerted: false,
            sneaking: false,
            dead: false,
            death_limbs: 0,
            death_cause: 0,
            combat_target: None,
            ai_package: 0,
            ai_flags: 0,
            factions: Vec::new(),
        }
    }

    pub fn set_value(&mut self, index: u8, value: f32, is_base: bool) {
        if is_base {
            self.base_values.insert(index, value);
        } else {
            self.values.insert(index, value);
        }
    }

    pub fn get_value(&self, index: u8) -> f32 {
        self.values.get(&index).copied().unwrap_or(0.0)
    }

    pub fn to_new_packet(&self) -> Packet {
        Packet::ActorNew {
            id: self.id(),
            ref_id: self.ref_data.ref_id,
            base_id: self.ref_data.base_id,
            values: self.values.clone(),
            base_values: self.base_values.clone(),
            race: self.race,
            age: self.age,
            idle: self.idle_anim,
            moving: self.moving_anim,
            moving_xy: self.moving_xy,
            weapon: self.weapon_anim,
            female: self.female,
            alerted: self.alerted,
            sneaking: self.sneaking,
            dead: self.dead,
            death_limbs: self.death_limbs,
            death_cause: self.death_cause,
            scale: self.object.scale,
        }
    }
}

impl GameObject for Actor {
    fn id(&self) -> NetworkID { self.container.id() }
    fn kind(&self) -> ObjectKind { ObjectKind::Actor }
    fn kind_mask(&self) -> u32 {
        ObjectKind::Object as u32
            | ObjectKind::ItemList as u32
            | ObjectKind::Container as u32
            | ObjectKind::Actor as u32
    }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl std::ops::Deref for Actor {
    type Target = Container;
    fn deref(&self) -> &Container { &self.container }
}

impl std::ops::DerefMut for Actor {
    fn deref_mut(&mut self) -> &mut Container { &mut self.container }
}

// ═══════════════════════════════════════════════════════════════
// Player — human-controlled character
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Player {
    pub actor: Actor,
    pub controls: HashMap<u8, (u8, bool)>,
    pub respawn_cell: u32,
    pub console_enabled: bool,
    pub attached_windows: Vec<NetworkID>,
}

impl Player {
    pub fn new(id: NetworkID, ref_id: u32, base_id: u32, cell: u32) -> Self {
        Player {
            actor: Actor::new(id, ref_id, base_id, cell),
            controls: HashMap::new(),
            respawn_cell: cell,
            console_enabled: false,
            attached_windows: Vec::new(),
        }
    }

    pub fn to_new_packet(&self) -> Packet {
        Packet::PlayerNew {
            id: self.id(),
            ref_id: self.ref_data.ref_id,
            base_id: self.ref_data.base_id,
            controls: self.controls.clone(),
            scale: self.object.scale,
        }
    }
}

impl GameObject for Player {
    fn id(&self) -> NetworkID { self.actor.id() }
    fn kind(&self) -> ObjectKind { ObjectKind::Player }
    fn kind_mask(&self) -> u32 {
        ObjectKind::Object as u32
            | ObjectKind::ItemList as u32
            | ObjectKind::Container as u32
            | ObjectKind::Actor as u32
            | ObjectKind::Player as u32
    }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl std::ops::Deref for Player {
    type Target = Actor;
    fn deref(&self) -> &Actor { &self.actor }
}

impl std::ops::DerefMut for Player {
    fn deref_mut(&mut self) -> &mut Actor { &mut self.actor }
}
