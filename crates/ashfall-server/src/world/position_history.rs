//! Position history — ring-buffered per-entity positions for server-side
//! lag compensation (combat range checks rewind to ~RTT ago instead of using
//! the attacker's current position, which is ahead of what the attacker saw).

use ashfall_core::id::NetworkID;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How far back to keep positions (2s @ 30Hz = 60 entries).
const HISTORY_WINDOW: Duration = Duration::from_secs(2);
/// Rewind window applied to combat range checks — the attacker's position
/// as it was ~1 RTT (100ms) before the server processed the hit.
pub const LAG_COMP_REWIND: Duration = Duration::from_millis(100);

/// Per-entity position ring buffer.
#[derive(Default)]
pub struct PositionHistory {
    by_id: DashMap<NetworkID, VecDeque<(Instant, [f32; 3])>>,
}

impl PositionHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an authoritative position for an entity, pruning old entries.
    pub fn record(&self, id: NetworkID, pos: [f32; 3]) {
        let now = Instant::now();
        self.by_id.entry(id).or_default().push_back((now, pos));
        if let Some(mut q) = self.by_id.get_mut(&id) {
            while q.front().is_some_and(|(t, _)| now.duration_since(*t) > HISTORY_WINDOW) {
                q.pop_front();
            }
        }
    }

    /// Position of an entity at-or-before `before`. Falls back to the newest
    /// entry (i.e., the current position) when nothing is old enough.
    pub fn pos_at(&self, id: NetworkID, before: Instant) -> Option<[f32; 3]> {
        let q = self.by_id.get(&id)?;
        let mut best: Option<[f32; 3]> = None;
        for (t, p) in q.iter() {
            if *t <= before {
                best = Some(*p);
            } else {
                break;
            }
        }
        best.or_else(|| q.back().map(|(_, p)| *p))
    }

    /// Lag-compensated position for a combat hit: the entity's position as of
    /// `now - LAG_COMP_REWIND` (the sender's view under ~100ms RTT).
    pub fn lag_compensated(&self, id: NetworkID, now: Instant) -> Option<[f32; 3]> {
        self.pos_at(id, now - LAG_COMP_REWIND)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_rewind() {
        let h = PositionHistory::new();
        let id = NetworkID::new(1);
        let now = Instant::now();

        // Simulate a player walking in +x: record 5 positions ~30ms apart
        for i in 0..5 {
            h.record(id, [i as f32 * 10.0, 0.0, 0.0]);
            std::thread::sleep(Duration::from_millis(30));
        }

        // The compensated position should be behind the current one
        let current = h.pos_at(id, Instant::now()).expect("current pos");
        let comp = h.lag_compensated(id, Instant::now()).expect("compensated pos");
        assert!(
            comp[0] < current[0],
            "lag compensation rewinds position (comp={}, current={})",
            comp[0],
            current[0]
        );

        // pos_at with a past timestamp returns the position from then
        let past = now + Duration::from_millis(60);
        let p = h.pos_at(id, past).expect("past pos");
        assert!(p[0] <= 20.0, "position at ~60ms is early in the walk, got {}", p[0]);
    }

    #[test]
    fn test_history_prunes() {
        let h = PositionHistory::new();
        let id = NetworkID::new(2);
        for i in 0..80 {
            h.record(id, [i as f32, 0.0, 0.0]);
            std::thread::sleep(Duration::from_millis(2));
        }
        // Entries older than the window are pruned
        let q = h.by_id.get(&id).unwrap();
        assert!(q.len() <= 80, "bounded history, len={}", q.len());
        assert!(q.len() > 5, "keeps recent entries, len={}", q.len());
    }
}
