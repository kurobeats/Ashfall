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
use std::sync::{LazyLock, Mutex};

/// The local player's ref id (vaultmp convention) — never an NPC spawn.
pub const LOCAL_PLAYER_REF: u32 = 0x14;

/// Ref-id offset on TESObjectREFR (xFOSE GameForms.h + vaultmp-extended,
/// two-tool verified in scripts/re). The actor-collector reads it to get the
/// form id from the actor pointer the AI predicate hands us.
pub const REFR_REFID_OFFSET: usize = 0x0C;

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

// ── live collection (fed by the AI-predicate detour) ──

/// Actors the engine has processed since the last flush (form ids). The
/// detour's thunk calls [`collect_actor_ptr`] once per actor per frame — the
/// engine's own processing list, no ProcessLists layout needed.
static CURRENT: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
/// Previous flush snapshot; diffing CURRENT against it yields spawns/removes.
static LAST: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Reset the collector state (tests).
pub fn reset_known() {
    CURRENT.lock().unwrap().clear();
    LAST.lock().unwrap().clear();
}

/// Called (via the x86 thunk) from the AI-predicate detour: the engine is
/// processing this actor right now. Reads the form id from the refr.
///
/// # Safety
/// `actor` must be a live TESObjectREFR pointer in the game process.
pub unsafe fn collect_actor_ptr(actor: usize) {
    if actor == 0 {
        return;
    }
    let ref_id =
        crate::hooks::vtable::read_field::<u32>(actor as *mut u8, REFR_REFID_OFFSET);
    if ref_id == 0 {
        return;
    }
    CURRENT.lock().unwrap().insert(ref_id);
}

/// Diff CURRENT against the last snapshot and emit spawn/remove event frames.
/// Returns (kind, ref_id) pairs (0 = spawn, 1 = remove) so tests can assert
/// without touching the pipe queue.
///
/// Call at 10 Hz (STR `cDelayBetweenSnapshots`): the collector accumulates a
/// frame's worth of processed actors, this turns the delta into events.
pub fn flush_npc_diff() -> Vec<(u8, u32)> {
    use ashfall_core::event::{encode_npc_remove_event, encode_npc_spawn_event, NpcRemoveEvent, NpcSpawnEvent};
    let current: Vec<u32> = std::mem::take(&mut *CURRENT.lock().unwrap()).into_iter().collect();
    let mut out = Vec::new();

    // ponytail: an empty window is a processing gap (idle frame), not a
    // mass despawn — never invent removals from silence. The server also
    // culls on UpdateContext leave, so missed removals self-heal.
    if current.is_empty() {
        return out;
    }

    let mut last = LAST.lock().unwrap();
    let (added, removed) = npc_diff(&mut last, &current);

    for ref_id in added {
        let e = NpcSpawnEvent {
            ref_id,
            base_id: crate::hooks::get_base(ref_id),
            pos: crate::hooks::get_pos(ref_id),
            cell: crate::hooks::get_cell(ref_id),
        };
        crate::network::push_event_frame(encode_npc_spawn_event(&e));
        out.push((0, ref_id));
    }
    for ref_id in removed {
        crate::network::push_event_frame(encode_npc_remove_event(&NpcRemoveEvent { ref_id }));
        out.push((1, ref_id));
    }
    out
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

    #[test]
    fn test_collect_and_flush_emits_events() {
        use ashfall_core::event::NpcSpawnEvent;
        reset_known();

        // The engine processes two actors this frame (formIDs from the fake
        // object's refid field).
        let mut obj_a = vec![0u8; 32];
        let mut obj_b = vec![0u8; 32];
        unsafe {
            crate::hooks::vtable::write_field::<u32>(obj_a.as_mut_ptr(), REFR_REFID_OFFSET, 0x100);
            crate::hooks::vtable::write_field::<u32>(obj_b.as_mut_ptr(), REFR_REFID_OFFSET, 0x101);
            collect_actor_ptr(obj_a.as_mut_ptr() as usize);
            collect_actor_ptr(obj_b.as_mut_ptr() as usize);
        }

        let events = flush_npc_diff();
        assert_eq!(events.len(), 2, "both NPCs spawn");
        let mut refs: Vec<u32> = events
            .iter()
            .filter_map(|(kind, id)| if *kind == 0 { Some(*id) } else { None })
            .collect();
        refs.sort_unstable(); // HashSet order is not deterministic
        assert_eq!(refs, vec![0x100, 0x101]);

        // Next flush with no new actors → nothing (empty window = idle).
        assert!(flush_npc_diff().is_empty());

        // Actor leaves processing (while another is active) → remove event.
        let mut obj_a2 = vec![0u8; 32];
        unsafe {
            crate::hooks::vtable::write_field::<u32>(obj_a2.as_mut_ptr(), REFR_REFID_OFFSET, 0x100);
            collect_actor_ptr(obj_a2.as_mut_ptr() as usize);
        }
        let events = flush_npc_diff();
        assert_eq!(events.len(), 1, "0x101 removed, 0x100 still present");
        assert_eq!(events[0], (1, 0x101));
    }
}
