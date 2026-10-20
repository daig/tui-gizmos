//! Exposure: which brightness becomes black and which becomes full.

use std::time::Duration;

use crate::look::{brightness, luma};
use crate::{Look, Picture};

/// How a picture's brightness maps onto the texture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    /// Luminance in `0.0..=1.0` at or below which the picture is black.
    pub black: f32,
    /// Luminance at or above which the picture is full brightness.
    pub white: f32,
    /// Brightness, on the scale `black..white` maps to `0..1`, above which
    /// dots appear.
    pub dots: f32,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            black: 0.0,
            white: 1.0,
            dots: 0.5,
        }
    }
}

impl Levels {
    /// Measures `picture`: black at the look's `black_percentile` of
    /// luminance (at most `max_black`), white at its `white_percentile`, and
    /// the dot threshold so that `dot_coverage` of the picture is above it.
    #[must_use]
    pub fn measure(picture: &Picture, look: &Look) -> Self {
        let pixels = picture.width() * picture.height();
        if pixels == 0 {
            return Self::default();
        }
        // About 64k samples is plenty for percentiles.
        let stride = (pixels / 65_536).max(1);
        let rgb = picture.rgb();
        let samples = || {
            (0..pixels)
                .step_by(stride)
                .map(|i| [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]].map(|v| f32::from(v) / 255.0))
        };
        let lumas = Histogram::of(samples().map(luma));
        let black = lumas.percentile(look.black_percentile).min(look.max_black);
        let white = lumas.percentile(look.white_percentile).max(black + 0.1);
        let mut levels = Self {
            black,
            white,
            dots: 0.5,
        };
        let bright = Histogram::of(samples().map(|c| brightness(look.normalize(c, levels))));
        let (lo, hi) = look.dot_threshold;
        levels.dots = bright.percentile(1.0 - look.dot_coverage).clamp(lo, hi.max(lo));
        levels
    }

    fn lerp(self, to: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            black: mix(self.black, to.black),
            white: mix(self.white, to.white),
            dots: mix(self.dots, to.dots),
        }
    }
}

/// A 256-bin histogram of values in `0.0..=1.0`.
struct Histogram {
    bins: [u32; 256],
    n: u32,
}

impl Histogram {
    fn of(values: impl Iterator<Item = f32>) -> Self {
        let mut h = Self { bins: [0; 256], n: 0 };
        for v in values {
            h.bins[((v * 255.0).round().max(0.0) as usize).min(255)] += 1;
            h.n += 1;
        }
        h
    }

    /// The value below which a share `p` of the values lie.
    fn percentile(&self, p: f32) -> f32 {
        let target = (p.clamp(0.0, 1.0) * self.n as f32).ceil().max(1.0) as u32;
        let mut seen = 0;
        self.bins
            .iter()
            .position(|&count| {
                seen += count;
                seen >= target
            })
            .unwrap_or(255) as f32
            / 255.0
    }
}

/// Levels that follow a video smoothly, so exposure doesn't pump from frame
/// to frame. Keep one in your app state and feed it every frame.
#[derive(Debug, Clone)]
pub struct AutoLevels {
    levels: Option<Levels>,
    time_constant: Duration,
}

impl AutoLevels {
    /// Levels that move a fraction `1 - e^(-dt/τ)` of the way to each new
    /// measurement, where τ is `time_constant`.
    #[must_use]
    pub const fn new(time_constant: Duration) -> Self {
        Self {
            levels: None,
            time_constant,
        }
    }

    /// Measures `picture` and returns the smoothed levels.
    pub fn update(&mut self, picture: &Picture, look: &Look, dt: Duration) -> Levels {
        let measured = Levels::measure(picture, look);
        let levels = match self.levels {
            None => measured,
            Some(_) if self.time_constant.is_zero() => measured,
            Some(current) => {
                let t = 1.0 - (-dt.as_secs_f32() / self.time_constant.as_secs_f32()).exp();
                current.lerp(measured, t)
            }
        };
        self.levels = Some(levels);
        levels
    }

    /// The current levels, once a frame has been seen.
    #[must_use]
    pub const fn get(&self) -> Option<Levels> {
        self.levels
    }
}

impl Default for AutoLevels {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_percentiles() {
        // A ramp from 0 to 255 across 256 pixels.
        let px: Vec<u8> = (0..=255u8).flat_map(|v| [v, v, v]).collect();
        let pic = Picture::new(256, 1, &px).unwrap();
        let look = Look {
            black_percentile: 0.25,
            max_black: 1.0,
            white_percentile: 0.75,
            ..Look::DEFAULT
        };
        let l = Levels::measure(&pic, &look);
        assert!((l.black - 0.25).abs() < 0.01 && (l.white - 0.75).abs() < 0.01, "{l:?}");
        // The black point is capped.
        let l = Levels::measure(&pic, &Look { max_black: 0.1, ..look });
        assert!((l.black - 0.1).abs() < 1e-6, "{l:?}");
    }

    #[test]
    fn flat_picture_keeps_a_range() {
        let px = [128u8; 300];
        let l = Levels::measure(&Picture::new(10, 10, &px).unwrap(), &Look::DEFAULT);
        assert!(l.white >= l.black + 0.1, "{l:?}");
    }

    #[test]
    fn dot_threshold_follows_coverage() {
        let px: Vec<u8> = (0..=255u8).flat_map(|v| [v, v, v]).collect();
        let pic = Picture::new(256, 1, &px).unwrap();
        let look = Look {
            black_percentile: 0.0,
            dot_coverage: 0.25,
            dot_threshold: (0.0, 1.0),
            saturation: 1.0,
            ..Look::DEFAULT
        };
        let l = Levels::measure(&pic, &look);
        assert!((l.dots - 0.75).abs() < 0.02, "{l:?}");
        // Clamped to the look's range.
        let l = Levels::measure(
            &pic,
            &Look {
                dot_threshold: (0.0, 0.5),
                ..look
            },
        );
        assert!((l.dots - 0.5).abs() < 1e-6, "{l:?}");
    }

    #[test]
    fn auto_levels_ease_towards_new_measurements() {
        let dark = [10u8; 300];
        let bright = [250u8; 300];
        let look = Look {
            white_percentile: 0.5,
            ..Look::DEFAULT
        };
        let mut auto = AutoLevels::new(Duration::from_secs(1));
        let first = auto.update(&Picture::new(10, 10, &dark).unwrap(), &look, Duration::ZERO);
        let next = auto.update(
            &Picture::new(10, 10, &bright).unwrap(),
            &look,
            Duration::from_millis(100),
        );
        let target = Levels::measure(&Picture::new(10, 10, &bright).unwrap(), &look);
        assert!(
            next.white > first.white && next.white < target.white,
            "{first:?} {next:?} {target:?}"
        );
    }
}
