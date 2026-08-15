//! Object type bitmask hierarchy — mirrors ReferenceTypes.hpp.

use crate::id::NetworkID;
use std::any::Any;

/// Bitmask type identifiers for the object hierarchy.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Reference = 0x0000_0001,
    Object = 0x0000_0002,
    ItemList = 0x0000_0004,
    Item = 0x0000_0008,
    Container = 0x0000_0010,
    Actor = 0x0000_0020,
    Player = 0x0000_0040,
    Window = 0x0000_0080,
    Button = 0x0000_0100,
    Text = 0x0000_0200,
    Edit = 0x0000_0400,
    Checkbox = 0x0000_0800,
    RadioButton = 0x0000_1000,
    ListItem = 0x0000_2000,
    List = 0x0000_4000,
}

impl ObjectKind {
    #[inline]
    pub fn mask(self) -> u32 {
        self as u32
    }

    /// Check if this kind is or descends from `other`.
    #[inline]
    pub fn is_kind(self, other: ObjectKind) -> bool {
        match other {
            ObjectKind::Reference => self.mask() & ALL_REFERENCES != 0,
            ObjectKind::Object => self.mask() & ALL_OBJECTS != 0,
            ObjectKind::ItemList => self.mask() & ALL_ITEMLISTS != 0,
            ObjectKind::Container => self.mask() & ALL_CONTAINERS != 0,
            ObjectKind::Actor => self.mask() & ALL_ACTORS != 0,
            ObjectKind::Item => self.mask() & (ObjectKind::Item as u32) != 0,
            ObjectKind::Player => self.mask() & (ObjectKind::Player as u32) != 0,
            ObjectKind::Window => self.mask() & ALL_WINDOWS != 0,
            other => self == other,
        }
    }
}

// Composite bitmasks for subtype checks.
pub const ALL_REFERENCES: u32 = 0x0000_007F;
pub const ALL_OBJECTS: u32 = 0x0000_007E;
pub const ALL_ITEMLISTS: u32 = 0x0000_0074;
pub const ALL_CONTAINERS: u32 = 0x0000_0070;
pub const ALL_ACTORS: u32 = 0x0000_0060;
pub const ALL_WINDOWS: u32 = 0x0000_7F80;

/// Core trait for all game objects.
///
/// Every game entity (Reference, Object, Item, Actor, Player, Window...) implements this.
pub trait GameObject: Any + Send + Sync {
    fn id(&self) -> NetworkID;
    fn kind(&self) -> ObjectKind;
    fn kind_mask(&self) -> u32;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// Convenience downcast.
impl dyn GameObject {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

// Reason codes for disconnect (matches Data.hpp).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Kick = 0,
    Ban = 1,
    Error = 2,
    Denied = 3,
    Quit = 4,
    None = 5,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_masks() {
        assert_eq!(ObjectKind::Actor.mask(), 0x20);
        assert_eq!(ObjectKind::Player.mask(), 0x40);
        assert_eq!(ObjectKind::Item.mask(), 0x8);
    }

    #[test]
    fn actor_hierarchy() {
        // Actor descends from Reference, Object, Container, ItemList — and itself
        assert!(ObjectKind::Actor.is_kind(ObjectKind::Reference));
        assert!(ObjectKind::Actor.is_kind(ObjectKind::Object));
        assert!(ObjectKind::Actor.is_kind(ObjectKind::Container));
        assert!(ObjectKind::Actor.is_kind(ObjectKind::ItemList));
        assert!(ObjectKind::Actor.is_kind(ObjectKind::Actor));
        // Actor is NOT a Player or Item
        assert!(!ObjectKind::Actor.is_kind(ObjectKind::Player));
        assert!(!ObjectKind::Actor.is_kind(ObjectKind::Item));
    }

    #[test]
    fn player_hierarchy() {
        assert!(ObjectKind::Player.is_kind(ObjectKind::Actor));
        assert!(ObjectKind::Player.is_kind(ObjectKind::Reference));
        assert!(ObjectKind::Player.is_kind(ObjectKind::Object));
        assert!(!ObjectKind::Player.is_kind(ObjectKind::Window));
    }

    #[test]
    fn item_and_window() {
        assert!(ObjectKind::Item.is_kind(ObjectKind::Item));
        assert!(!ObjectKind::Item.is_kind(ObjectKind::Container));
        assert!(ObjectKind::Window.is_kind(ObjectKind::Window));
        // UI sub-kinds descend from Window (ALL_WINDOWS = 0x7F80)
        assert!(ObjectKind::Button.is_kind(ObjectKind::Window));
        assert!(ObjectKind::Checkbox.is_kind(ObjectKind::Window));
        assert!(ObjectKind::List.is_kind(ObjectKind::Window));
        // windows are NOT actors / items / plain references
        assert!(!ObjectKind::Window.is_kind(ObjectKind::Actor));
        assert!(!ObjectKind::Window.is_kind(ObjectKind::Reference));
        assert!(!ObjectKind::Window.is_kind(ObjectKind::Item));
    }

    #[test]
    fn exact_match_fallback() {
        // non-hierarchy kinds match only themselves
        assert!(ObjectKind::Checkbox.is_kind(ObjectKind::Checkbox));
        assert!(!ObjectKind::Checkbox.is_kind(ObjectKind::Button));
    }
}
