//! String dictionary compression (SkyrimTogetherReborn StringCache pattern).
//!
//! Names, cell names, and chat text repeat constantly on the wire (every
//! cell entry re-sends the same object names). Instead of re-sending the
//! bytes, the server binds each string to a u16 id per connection; the first
//! transmission carries id + bytes, later ones just the id.
//!
//! The server is the only id assigner. Clients always send `Plain` strings;
//! the server interns them on send (per-recipient, so every session learns
//! the same id for the same string) and clients resolve `Inline`/`Id` on
//! receive against a table built from what the server sent them.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bidirectional string ↔ id dictionary (server: per-session; client: one).
#[derive(Debug, Default, Clone)]
pub struct StringTable {
    by_id: Vec<String>,
    by_str: HashMap<String, u16>,
}

impl StringTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a string's id, or assign a fresh one.
    pub fn intern(&mut self, value: &str) -> u16 {
        if let Some(&id) = self.by_str.get(value) {
            return id;
        }
        let id = self.by_id.len() as u16;
        self.by_id.push(value.to_string());
        self.by_str.insert(value.to_string(), id);
        id
    }

    /// Resolve an id back to its string.
    /// Empty placeholders (sparse ids from `Inline` registration) read as missing.
    pub fn lookup(&self, id: u16) -> Option<&str> {
        let s = self.by_id.get(id as usize)?;
        if s.is_empty() {
            None
        } else {
            Some(s.as_str())
        }
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Wire form of a string: full value first, id-only afterwards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CachedString {
    /// Unbound string (client → server, or server before interning).
    /// Serialized as its own variant — receivers treat it as uncached.
    Plain(String),
    /// First transmission on this connection: the assigned id + the bytes.
    Inline { id: u16, value: String },
    /// Previously sent on this connection: id only.
    Id(u16),
}

impl CachedString {
    /// Client-side / inbound resolution: return the string, and for `Inline`
    /// register the id → value mapping so later `Id` references resolve.
    pub fn resolve(&self, table: &mut StringTable) -> String {
        match self {
            CachedString::Plain(s) => s.clone(),
            CachedString::Inline { id, value } => {
                if table.lookup(*id).is_none() {
                    table.by_id.resize(*id as usize + 1, String::new());
                    table.by_id[*id as usize] = value.clone();
                    table.by_str.insert(value.clone(), *id);
                }
                value.clone()
            }
            CachedString::Id(id) => table.lookup(*id).unwrap_or("").to_string(),
        }
    }

    /// Server-side: bind a plain string to `table` before sending — the
    /// first sight of the string goes out as `Inline` (id + bytes), repeats
    /// as `Id` only.
    pub fn intern(table: &mut StringTable, value: String) -> CachedString {
        match table.by_str.get(&value) {
            Some(&id) => CachedString::Id(id),
            None => {
                let id = table.intern(&value);
                CachedString::Inline { id, value }
            }
        }
    }
}

impl From<&str> for CachedString {
    fn from(s: &str) -> Self {
        CachedString::Plain(s.to_string())
    }
}

impl From<String> for CachedString {
    fn from(s: String) -> Self {
        CachedString::Plain(s)
    }
}

impl From<CachedString> for String {
    fn from(cs: CachedString) -> Self {
        match cs {
            CachedString::Plain(s) | CachedString::Inline { value: s, .. } => s,
            CachedString::Id(_) => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_assigns_stable_ids() {
        let mut t = StringTable::new();
        assert_eq!(t.intern("hello"), 0);
        assert_eq!(t.intern("world"), 1);
        assert_eq!(t.intern("hello"), 0, "repeat returns the same id");
        assert_eq!(t.lookup(0), Some("hello"));
        assert_eq!(t.lookup(1), Some("world"));
        assert_eq!(t.lookup(99), None);
    }

    #[test]
    fn test_intern_plain_then_id_only() {
        let mut table = StringTable::new();
        let first = CachedString::intern(&mut table, "Vault101".to_string());
        assert!(matches!(&first, CachedString::Inline { id: 0, value } if value == "Vault101"));
        let second = CachedString::intern(&mut table, "Vault101".to_string());
        assert_eq!(second, CachedString::Id(0), "repeat sends id only");
    }

    #[test]
    fn test_client_resolve_builds_table() {
        let mut t = StringTable::new();
        // Server first-sight: Inline{id=3, "Vault101"} (ids aren't dense).
        let s = CachedString::Inline {
            id: 3,
            value: "Vault101".into(),
        };
        assert_eq!(s.resolve(&mut t), "Vault101");
        assert_eq!(t.lookup(3), Some("Vault101"));
        // Later: Id resolves against the learned table.
        assert_eq!(CachedString::Id(3).resolve(&mut t), "Vault101");
        assert_eq!(
            CachedString::Id(7).resolve(&mut t),
            "",
            "unknown id → empty"
        );
        // Plain passes through without caching.
        assert_eq!(CachedString::Plain("raw".into()).resolve(&mut t), "raw");
        assert_eq!(t.lookup(0), None, "plain never registered");
    }

    #[test]
    fn test_wire_roundtrip() {
        let cases = vec![
            CachedString::Plain("x".into()),
            CachedString::Inline {
                id: 42,
                value: "Vault101".into(),
            },
            CachedString::Id(42),
        ];
        for cs in cases {
            let bytes = postcard::to_stdvec(&cs).unwrap();
            let back: CachedString = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(back, cs);
        }
        // Id-only must be smaller than Inline.
        let id = postcard::to_stdvec(&CachedString::Id(42)).unwrap();
        let inline = postcard::to_stdvec(&CachedString::Inline {
            id: 42,
            value: "Vault101".into(),
        })
        .unwrap();
        assert!(id.len() < inline.len());
    }
}
