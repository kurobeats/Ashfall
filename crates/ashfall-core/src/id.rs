//! NetworkID newtype — wraps RakNet/u64 identifier.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Globally unique identifier for a game object.
///
/// Maps to `RakNet::NetworkID` (u64) in the original C++.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NetworkID(u64);

impl NetworkID {
    pub const NULL: Self = NetworkID(0);

    #[inline]
    pub fn new(id: u64) -> Self {
        NetworkID(id)
    }

    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl From<u64> for NetworkID {
    fn from(id: u64) -> Self {
        NetworkID(id)
    }
}

impl From<NetworkID> for u64 {
    fn from(id: NetworkID) -> Self {
        id.0
    }
}

impl fmt::Display for NetworkID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for NetworkID {
    fn default() -> Self {
        NetworkID::NULL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_id_newtype() {
        let n = NetworkID::new(42);
        assert_eq!(n.as_u64(), 42);
        assert!(!n.is_null());
        assert!(NetworkID::NULL.is_null());
        assert_eq!(NetworkID::default(), NetworkID::NULL);
        assert_eq!(NetworkID::from(7u64), NetworkID::new(7));
        assert_eq!(u64::from(NetworkID::new(7)), 7);
        assert_eq!(format!("{n}"), "42");
    }

    #[test]
    fn network_id_equality_and_hash() {
        assert_eq!(NetworkID::new(1), NetworkID::new(1));
        assert_ne!(NetworkID::new(1), NetworkID::new(2));
    }
}
