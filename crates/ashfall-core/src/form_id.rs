//! FormID — universal Gamebryo identifier for all world objects.
//!
//! Matches TESForm::formID (u32). The top byte encodes the mod index;
//! the lower 3 bytes encode the object index within that mod.

use serde::{Deserialize, Serialize};

/// Gamebryo FormID — unique per object across Fallout 3 / New Vegas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FormID(u32);

impl FormID {
    pub const NULL: Self = FormID(0);

    #[inline]
    pub fn new(id: u32) -> Self {
        FormID(id)
    }

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Mod index (top byte).
    #[inline]
    pub fn mod_index(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// Object index within the mod (lower 24 bits).
    #[inline]
    pub fn object_id(self) -> u32 {
        self.0 & 0x00FF_FFFF
    }
}

impl From<u32> for FormID {
    fn from(id: u32) -> Self {
        FormID(id)
    }
}

impl From<FormID> for u32 {
    fn from(id: FormID) -> Self {
        id.0
    }
}

impl std::fmt::Display for FormID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

impl Default for FormID {
    fn default() -> Self {
        FormID::NULL
    }
}

/// Lightweight FormID + transform entry for cell snapshot sync.
///
/// Sent as a batch on cell entry instead of full object state.
/// Client creates placeholder objects; server fills details on request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormIDSync {
    pub form_id: FormID,
    pub pos: [f32; 3],
    pub angle: [f32; 3],
    pub scale: f32,
    /// Bit flags: 0x01 = initially disabled, 0x02 = deleted, 0x04 = persistent, etc.
    pub flags: u32,
}

impl FormIDSync {
    pub fn new(form_id: FormID, pos: [f32; 3], angle: [f32; 3], scale: f32, flags: u32) -> Self {
        FormIDSync {
            form_id,
            pos,
            angle,
            scale,
            flags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_id_parts() {
        let f = FormID::new(0x01AB_CDEF);
        assert_eq!(f.mod_index(), 0x01);
        assert_eq!(f.object_id(), 0xAB_CDEF);
        assert_eq!(f.as_u32(), 0x01AB_CDEF);
        assert!(!f.is_null());
        assert!(FormID::NULL.is_null());
        assert_eq!(FormID::default(), FormID::NULL);
    }

    #[test]
    fn form_id_conversions_and_display() {
        assert_eq!(FormID::from(0x12_3456), FormID::new(0x12_3456));
        assert_eq!(u32::from(FormID::new(0x77)), 0x77);
        assert_eq!(format!("{}", FormID::new(0x0A0B0C0D)), "0A0B0C0D");
    }

    #[test]
    fn form_id_sync_new() {
        let s = FormIDSync::new(FormID::new(1), [0.0; 3], [0.0; 3], 1.0, 0x01);
        assert_eq!(s.flags, 0x01);
        assert_eq!(s.form_id, FormID::new(1));
    }
}
