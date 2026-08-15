//! Client-side position interpolation — render-behind buffer with
//! extrapolation, ported from mojave-online's `fnvmp/game/interpolation.cpp`
//! (MIT).
//!
//! Instead of blending toward the newest sample (which arrives late and
//! stutters on jitter), the renderer looks at `now - INTERP_DELAY`:
//!   - between two samples → lerp (smooth, absorbs one dropped packet)
//!   - past the newest → extrapolate along the estimated velocity
//!   - past `EXTRAP_TIMEOUT` (no packets for 500ms) → freeze at last position

use std::sync::OnceLock;

/// Render delay: 2 packet intervals at the 30Hz snapshot rate.
pub const INTERP_DELAY: f32 = 2.0 / 30.0;
/// Extrapolation ceiling — freeze when the newest sample is older than this.
pub const EXTRAP_TIMEOUT: f32 = 0.5;
/// Ring size: enough to cover delay + jitter.
const BUFFER_SIZE: usize = 5;

/// Monotonic clock: seconds since first use of this process.
fn now_seconds() -> f32 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    let start = *START.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs_f32()
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    pos: [f32; 3],
    t: f32,
}

/// Per-entity interpolation state.
#[derive(Debug, Clone, Default)]
pub struct InterpBuffer {
    /// Oldest → newest, capped at `BUFFER_SIZE`.
    samples: Vec<Sample>,
    /// Estimated velocity (units/s) from the last two samples.
    vel: [f32; 3],
}

impl InterpBuffer {
    #[allow(dead_code)] // used by tests
    pub fn new() -> Self {
        Self::default()
    }

    /// True when no samples have been pushed yet.
    #[allow(dead_code)] // used by tests; renderer may use for early-out
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Push a position sample; updates the velocity estimate from the
    /// previous newest sample. `t` = monotonic seconds.
    pub fn push(&mut self, pos: [f32; 3], t: f32) {
        if let Some(newest) = self.samples.last() {
            let dt = t - newest.t;
            if dt > 0.001 {
                self.vel = [
                    (pos[0] - newest.pos[0]) / dt,
                    (pos[1] - newest.pos[1]) / dt,
                    (pos[2] - newest.pos[2]) / dt,
                ];
            }
        }
        self.samples.push(Sample { pos, t });
        if self.samples.len() > BUFFER_SIZE {
            self.samples.remove(0);
        }
    }

    /// Interpolated (or extrapolated) position at `render_time`, given the
    /// current clock `now`. Returns the frozen position when no data.
    pub fn render(&self, render_time: f32, now: f32) -> [f32; 3] {
        let Some(newest) = self.samples.last() else {
            return [0.0; 3];
        };
        let oldest = self.samples[0];

        // Case 1: single sample, or render_time behind the oldest → snap.
        if self.samples.len() == 1 || render_time <= oldest.t {
            return oldest.pos;
        }

        // Case 2: between two samples → lerp.
        if render_time <= newest.t {
            for w in self.samples.windows(2) {
                let (a, b) = (w[0], w[1]);
                if render_time >= a.t && render_time <= b.t {
                    let dt = b.t - a.t;
                    let tt = if dt > 0.0001 {
                        (render_time - a.t) / dt
                    } else {
                        0.0
                    };
                    return [
                        a.pos[0] + tt * (b.pos[0] - a.pos[0]),
                        a.pos[1] + tt * (b.pos[1] - a.pos[1]),
                        a.pos[2] + tt * (b.pos[2] - a.pos[2]),
                    ];
                }
            }
        }

        // Case 3: past the newest — extrapolate, or freeze past timeout.
        if now - newest.t > EXTRAP_TIMEOUT {
            return newest.pos;
        }
        let dt = render_time - newest.t;
        [
            newest.pos[0] + self.vel[0] * dt,
            newest.pos[1] + self.vel[1] * dt,
            newest.pos[2] + self.vel[2] * dt,
        ]
    }

    /// Convenience: push with the process-monotonic clock.
    pub fn push_now(&mut self, pos: [f32; 3]) {
        self.push(pos, now_seconds());
    }

    /// Convenience: render with the process-monotonic clock.
    pub fn render_now(&self) -> [f32; 3] {
        let now = now_seconds();
        self.render(now - INTERP_DELAY, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_samples_returns_origin() {
        let b = InterpBuffer::new();
        assert!(b.is_empty());
        assert_eq!(b.render(1.0, 1.0), [0.0; 3]);
    }

    #[test]
    fn single_sample_snaps() {
        let mut b = InterpBuffer::new();
        b.push([5.0, 0.0, 0.0], 1.0);
        assert_eq!(b.render(0.5, 1.0), [5.0, 0.0, 0.0]);
        assert_eq!(
            b.render(2.0, 2.5),
            [5.0, 0.0, 0.0],
            "no extrapolation with 1 sample"
        );
    }

    #[test]
    fn between_samples_lerps() {
        let mut b = InterpBuffer::new();
        b.push([0.0, 0.0, 0.0], 1.0);
        b.push([10.0, 0.0, 0.0], 2.0);
        // Render behind: t=1.5 → halfway.
        assert_eq!(b.render(1.5, 2.0), [5.0, 0.0, 0.0]);
        // At a sample boundary → exact.
        assert_eq!(b.render(1.0, 2.0), [0.0, 0.0, 0.0]);
        assert_eq!(b.render(2.0, 2.0), [10.0, 0.0, 0.0]);
    }

    #[test]
    fn extrapolates_past_newest_with_velocity() {
        let mut b = InterpBuffer::new();
        b.push([0.0, 0.0, 0.0], 1.0);
        b.push([10.0, 0.0, 0.0], 2.0); // vel = 10 u/s
                                       // Render 0.25s past the newest → 10 + 10*0.25 = 12.5.
        assert_eq!(b.render(2.25, 2.25), [12.5, 0.0, 0.0]);
    }

    #[test]
    fn freezes_after_extrap_timeout() {
        let mut b = InterpBuffer::new();
        b.push([0.0, 0.0, 0.0], 1.0);
        b.push([10.0, 0.0, 0.0], 2.0);
        // now = 2.6 → 0.6s since newest > 0.5 timeout → freeze at newest.
        assert_eq!(b.render(2.6, 2.6), [10.0, 0.0, 0.0]);
        assert_eq!(b.render(3.0, 3.0), [10.0, 0.0, 0.0]);
    }

    #[test]
    fn ring_caps_and_uses_newest() {
        let mut b = InterpBuffer::new();
        for i in 0..10u32 {
            b.push([i as f32, 0.0, 0.0], i as f32);
        }
        assert_eq!(b.samples.len(), BUFFER_SIZE);
        // Render exactly at the newest (t=9) → pos 9.
        assert_eq!(b.render(9.0, 9.0), [9.0, 0.0, 0.0]);
        // And the oldest kept is t=5.
        assert_eq!(b.samples[0].t, 5.0);
    }

    #[test]
    fn velocity_requires_positive_dt() {
        let mut b = InterpBuffer::new();
        b.push([0.0, 0.0, 0.0], 1.0);
        b.push([10.0, 0.0, 0.0], 1.0); // dt=0 → velocity stays 0
        assert_eq!(b.render(2.0, 2.0), [10.0, 0.0, 0.0]);
        assert_eq!(b.vel, [0.0; 3]);
    }
}
