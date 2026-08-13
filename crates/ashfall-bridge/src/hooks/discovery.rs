//! NPC discovery — the SkyrimTogetherReborn `VisitForms` pattern.
//!
//! STR's DiscoveryService enumerates the engine's active-actor list
//! (`ProcessLists::highActorHandleArray`), resolves each handle, and keeps a
//! seen-set of form ids. New forms → spawn; forms that vanished → remove.
//! Two filters matter:
//! - only refs with a live 3D node count (actually rendered, not stored);
//! - the local player (form 0x14) is never reported as an NPC.
//!
//! The enumeration itself is live-RE work on the Steam build (ProcessLists
//! layout — see docs/steam-re.md). This module is the diff half, pure and
//! unit-tested: feed it the observed ref ids each frame, it tells you what
//! spawned and what despawned. The bridge's per-frame hook (once RE'd) does:
//!
//! ```text
//! observed = enumerate_process_lists_actors();   // RE: Steam build
//! (added, removed) = npc_diff(&mut seen, &observed);
//! for id in added   { push EVENT_NPC_SPAWN   { ref_id: id, .. } }
//! for id in removed { push EVENT_NPC_REMOVE  { ref_id: id } }
//! ```

use std::collections::HashSet;

/// The local player's ref id (vaultmp convention) — never an NPC spawn.
pub const LOCAL_PLAYER_REF: u32 = 0x14;

/// Diff the current observed ref ids against the seen set.
/// Returns (added, removed); the seen set is updated in place.
/// Removals are returned in the same pass (STR dispatches them first so the
/// world can free entities before re-creating).
pub fn npc_diff(seen: &mut HashSet<u32>, observed: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut added = Vec::new();
    let mut removed = Vec::new();

    // New spawns.
    for &ref_id in observed {
        if ref_id == LOCAL_PLAYER_REF {
            continue;
        }
        if seen.insert(ref_id) {
            added.push(ref_id);
        }
    }

    // Gone actors: seen but no longer observed.
    let mut still_seen = HashSet::with_capacity(observed.len() + seen.len() / 4);
    for &ref_id in observed {
        still_seen.insert(ref_id);
    }
    for &ref_id in seen.iter() {
        if !still_seen.contains(&ref_id) {
            removed.push(ref_id);
        }
    }
    for ref_id in &removed {
        seen.remove(ref_id);
    }

    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_observation_all_added() {
        let mut seen = HashSet::new();
        let (added, removed) = npc_diff(&mut seen, &[0x100, 0x101, 0x102]);
        assert_eq!(added, vec![0x100, 0x101, 0x102]);
        assert!(removed.is_empty());
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn test_stable_set_no_events() {
        let mut seen = HashSet::new();
        npc_diff(&mut seen, &[0x100, 0x101]);
        let (added, removed) = npc_diff(&mut seen, &[0x100, 0x101]);
        assert!(added.is_empty());
        assert!(removed.is_empty());
    }

    #[test]
    fn test_spawn_and_despawn() {
        let mut seen = HashSet::new();
        npc_diff(&mut seen, &[0x100, 0x101]);
        // 0x101 left, 0x200 arrived.
        let (added, removed) = npc_diff(&mut seen, &[0x100, 0x200]);
        assert_eq!(added, vec![0x200]);
        assert_eq!(removed, vec![0x101]);
        assert_eq!(seen.len(), 2);
        // Re-appearing actor spawns again.
        let (added, _) = npc_diff(&mut seen, &[0x100, 0x200, 0x101]);
        assert_eq!(added, vec![0x101]);
    }

    #[test]
    fn test_player_never_reported() {
        let mut seen = HashSet::new();
        let (added, _) = npc_diff(&mut seen, &[LOCAL_PLAYER_REF, 0x100]);
        assert_eq!(added, vec![0x100], "local player filtered out");
        assert!(!seen.contains(&LOCAL_PLAYER_REF));
    }
}
