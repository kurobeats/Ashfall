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

/// FNV ActorProcessManager object address (AnhNVSE GameProcess.cpp:
/// `g_actorProcessManager = (ActorProcessManager*)0x011E0E80` — the object
/// lives at this fixed address; the game passes it as `this`, confirmed on
/// the GOG 1.4.0.525 binary via `mov ecx, 0x11e0e80` sites).
/// ponytail: FNV-only; never deref on FO3.
pub const FNV_ACTOR_PROCESS_MANAGER: usize = 0x011E_0E80;
/// First actor tier: `ActorList { tList<Actor> head; Node* tail }` — the
/// tList head is a ListNode { Actor* data@+0; Node* next@+4 }. Confirmed by
/// the manager's own count method reading `[this+4]` (0x977540).
/// Other tiers (lowActors @ +0x0C/+0x18, highActors @ +0x5C per AnhNVSE's
/// "needs recalc" header) are host-verify candidates — start with the
/// confirmed tier, extend once live-probed.
pub const FNV_FIRST_ACTOR_LIST: usize = FNV_ACTOR_PROCESS_MANAGER + 0x00;

/// Hard cap on list nodes (a corrupted `next` chain must not loop forever).
const LIST_NODE_CAP: usize = 4096;

/// Walk a tList chain: head.next → nodes { data@+0, next@+4 }, collecting
/// each actor's ref id (formID at +0x0C). `reader` abstracts memory access
/// so the walk is unit-testable.
pub fn walk_actor_list(head: usize, reader: impl Fn(usize) -> u32, out: &mut Vec<u32>) {
    let mut next = reader(head + 4) as usize;
    let mut guard = 0;
    while next != 0 && guard < LIST_NODE_CAP {
        guard += 1;
        let actor = reader(next) as usize;
        if actor != 0 {
            let ref_id = reader(actor + REFR_REFID_OFFSET);
            if ref_id != 0 && ref_id != LOCAL_PLAYER_REF {
                out.push(ref_id);
            }
        }
        next = reader(next + 4) as usize;
    }
}

/// Enumerate the FNV actor lists (the discovery source for FNV — replaces
/// the FO3 AI-predicate detour: the per-frame main-loop hook feeds this).
pub fn fnv_enumerate_actors(reader: impl Fn(usize) -> u32) -> Vec<u32> {
    let mut out = Vec::new();
    walk_actor_list(FNV_FIRST_ACTOR_LIST, reader, &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

/// Feed ref ids into the collector (called from the FNV frame hook each
/// frame — the 10 Hz flush diffs CURRENT against the last snapshot).
pub fn collect_ref_ids(ref_ids: &[u32]) {
    let mut current = CURRENT.lock().unwrap();
    for &r in ref_ids {
        current.insert(r);
    }
}

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
    fn test_fnv_actor_list_walk() {
        // Fake memory: manager → list head at FNV_FIRST_ACTOR_LIST.
        // Node layout: { data@+0, next@+4 }; head { data=0, next@+4 }.
        use std::collections::HashMap;
        let mut mem: HashMap<usize, u32> = HashMap::new();
        fn read(mem: &HashMap<usize, u32>, a: usize) -> u32 {
            mem.get(&a).copied().unwrap_or(0)
        }
        let head = FNV_FIRST_ACTOR_LIST;
        let n1 = 0x3000usize; // heap-ish node addresses
        let n2 = 0x3100usize;
        // Nodes: actor ptrs + next chain.
        mem.insert(n1, 0x5000); // node1.data = actor A
        mem.insert(n1 + 4, n2 as u32);
        mem.insert(n2, 0x5100); // node2.data = actor B
        mem.insert(n2 + 4, 0); // end of chain
        // Actors: refid at +0x0C (0x100 / player).
        mem.insert(0x5000 + REFR_REFID_OFFSET, 0x100);
        mem.insert(0x5100 + REFR_REFID_OFFSET, LOCAL_PLAYER_REF);
        // Head: data = 0 (unused), next = n1.
        mem.insert(head, 0);
        mem.insert(head + 4, n1 as u32);

        let actors = fnv_enumerate_actors(|a| read(&mem, a));
        assert_eq!(actors, vec![0x100], "player filtered, deduped, sorted");

        // Empty list → nothing.
        mem.remove(&(head + 4));
        let actors = fnv_enumerate_actors(|a| read(&mem, a));
        assert!(actors.is_empty());

        // Cyclic chain → cap saves us.
        mem.insert(head + 4, n1 as u32);
        mem.insert(n1 + 4, n1 as u32); // self-loop
        let actors = fnv_enumerate_actors(|a| read(&mem, a));
        assert!(actors.len() <= 1, "cycle bounded by node cap");
    }

    #[test]
    fn test_collect_ref_ids_feeds_diff() {
        reset_known();
        collect_ref_ids(&[0x200, 0x201]);
        let events = flush_npc_diff();
        assert_eq!(events.len(), 2, "both collected spawn");
        reset_known();
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
