//! Small, optional helpers that make gizmo values move nicely.
//!
//! Widgets are stateless; keep one of these in your app state, feed it raw
//! samples, and pass its output to the widget each frame.

use std::time::Duration;

/// Exponentially eases toward a target, so a needle swings instead of
/// teleporting between samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Damped {
    value: f64,
    time_constant: Duration,
}

impl Damped {
    /// Starts at `value`. `time_constant` is how long it takes to cover ~63%
    /// of the distance to a new target; 250 ms feels like a real needle.
    #[must_use]
    pub const fn new(value: f64, time_constant: Duration) -> Self {
        Self { value, time_constant }
    }

    /// Advances by `dt` toward `target` and returns the new value.
    pub fn update(&mut self, target: f64, dt: Duration) -> f64 {
        let tau = self.time_constant.as_secs_f64();
        let alpha = if tau <= 0.0 {
            1.0
        } else {
            1.0 - (-dt.as_secs_f64() / tau).exp()
        };
        if target.is_finite() {
            self.value += (target - self.value) * alpha;
        }
        self.value
    }

    /// The current value.
    #[must_use]
    pub const fn get(&self) -> f64 {
        self.value
    }
}

/// Remembers the highest value seen, holds it, then lets it fall back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakHold {
    peak: f64,
    held: Duration,
    hold: Duration,
    decay_per_sec: f64,
}

impl PeakHold {
    /// Holds a new peak for `hold`, then decays it by the factor
    /// `decay_per_sec` each second (e.g. `0.5` halves it every second).
    #[must_use]
    pub const fn new(hold: Duration, decay_per_sec: f64) -> Self {
        Self {
            peak: 0.0,
            held: Duration::ZERO,
            hold,
            decay_per_sec,
        }
    }

    /// Records a sample `dt` after the previous one and returns the peak.
    pub fn update(&mut self, sample: f64, dt: Duration) -> f64 {
        self.held += dt;
        if self.held > self.hold {
            self.peak *= self.decay_per_sec.powf(dt.as_secs_f64());
        }
        if sample >= self.peak {
            self.peak = sample;
            self.held = Duration::ZERO;
        }
        self.peak
    }

    /// The current peak.
    #[must_use]
    pub const fn get(&self) -> f64 {
        self.peak
    }
}
