//! The parameters that give a render its texture.

use crate::Levels;

/// An RGB colour.
pub type Rgb = [u8; 3];

/// Everything that shapes how a picture is turned into marks.
///
/// Brightness values below are on the picture's own scale after exposure:
/// 0 is its black point and 1 its white point (see [`Levels`]).
///
/// Start from [`Look::DEFAULT`] and override what you need:
///
/// ```
/// use tui_stipple::Look;
///
/// let sparser = Look { dot_coverage: 0.2, ..Look::DEFAULT };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// Luminance percentile that becomes black when levels are measured.
    pub black_percentile: f32,
    /// Upper limit on the measured black point, so a bright picture keeps
    /// its midtones.
    pub max_black: f32,
    /// Luminance percentile that becomes full brightness.
    pub white_percentile: f32,
    /// Colour saturation multiplier, applied first.
    pub saturation: f32,

    /// Background brightness at full picture brightness.
    pub base_gain: f32,
    /// Exponent shaping the background; below 1 lifts the shadows so dark
    /// detail stays visible.
    pub base_gamma: f32,
    /// Number of posterisation steps in the background.
    pub base_levels: f32,
    /// Extra saturation for the background, so shadows keep their hue
    /// instead of greying out.
    pub base_saturation: f32,

    /// Share of the picture, brightest first, that gets dots. The rest is
    /// left to the background. A dot's brightness is the average of its
    /// luminance and its strongest channel, lightly blurred so dots gather
    /// in regions rather than speckling every highlight.
    pub dot_coverage: f32,
    /// Lowest and highest brightness the coverage threshold may land on, so
    /// a dark picture doesn't fill with dots and a bright one keeps some.
    pub dot_threshold: (f32, f32),
    /// Width of the soft edge around the threshold.
    pub dot_edge: f32,
    /// Share of dots lit just above the threshold.
    pub dot_min: f32,
    /// Share of dots lit once brightness is `dot_ramp` above the threshold;
    /// below 1 so even flat bright areas keep shimmering.
    pub dot_max: f32,
    /// How far above the threshold dots reach `dot_max`.
    pub dot_ramp: f32,
    /// How decisively a dot takes its colour from the strongest channel:
    /// 1 picks channels in proportion to their strength, higher favours the
    /// strongest.
    pub hue_contrast: f32,
    /// Chance that a dot mixes in a second channel (becoming yellow, cyan or
    /// magenta) when that channel is as strong as the first.
    pub dot_mix: f32,
    /// Luminance above which a cell, if all its dots are this bright, becomes
    /// a solid block.
    pub clip: f32,
    /// Dot colours, indexed by channel bits: red = 1, green = 2, blue = 4.
    pub palette: [Rgb; 8],

    /// Chance of a dash in a dark cell without dots (0 by default; see
    /// [`ACCENTED`](Self::ACCENTED)).
    pub dash_density: f32,
    /// Picture luminance (before exposure) below which a cell counts as
    /// empty and gets no dashes; they reach full density at twice this.
    pub dash_floor: f32,
    /// Luminance above which dashes thin out, gone at twice this.
    pub dash_max: f32,
    /// Dash colour.
    pub dash_color: Rgb,
    /// Glyphs dashes are drawn with, picked at random.
    pub dash_glyphs: &'static [&'static str],

    /// Chance of a drip in a cell without dots, at full strength (0 by
    /// default; see [`ACCENTED`](Self::ACCENTED)).
    pub drip_density: f32,
    /// How strongly warm colour (red over green and blue) starts a drip.
    pub drip_gain: f32,
    /// How much of a drip's strength carries down each row.
    pub drip_decay: f32,
    /// Drip colour at full strength.
    pub drip_color: Rgb,
    /// Glyph drips are drawn with.
    pub drip_glyph: &'static str,

    /// Colour outside the picture.
    pub background: Rgb,
}

impl Look {
    /// The texture: a dark posterised base, with a screen of pure-primary
    /// dots over the bright parts and solid blocks where it is near white.
    pub const DEFAULT: Self = Self {
        black_percentile: 0.18,
        max_black: 0.3,
        white_percentile: 0.995,
        saturation: 1.3,

        base_gain: 0.25,
        base_gamma: 0.5,
        base_levels: 7.0,
        base_saturation: 1.6,

        dot_coverage: 0.35,
        dot_threshold: (0.3, 0.8),
        dot_edge: 0.03,
        dot_min: 0.5,
        dot_max: 0.85,
        dot_ramp: 0.2,
        hue_contrast: 1.5,
        dot_mix: 0.12,
        clip: 0.9,
        palette: [
            [0, 0, 0],
            [230, 30, 60],   // red
            [40, 245, 50],   // green
            [225, 240, 60],  // yellow
            [40, 40, 235],   // blue
            [225, 55, 210],  // magenta
            [50, 235, 225],  // cyan
            [230, 235, 215], // white
        ],

        dash_density: 0.0,
        dash_floor: 0.03,
        dash_max: 0.3,
        dash_color: [225, 55, 210],
        dash_glyphs: &["╴", "╶"],

        drip_density: 0.0,
        drip_gain: 2.5,
        drip_decay: 0.85,
        drip_color: [200, 20, 40],
        drip_glyph: "│",

        background: [0, 0, 0],
    };

    /// [`DEFAULT`](Self::DEFAULT) plus accents: magenta dashes through dark
    /// areas and dim red drips running down from warm colours.
    pub const ACCENTED: Self = Self {
        dash_density: 0.2,
        drip_density: 0.3,
        ..Self::DEFAULT
    };

    /// Applies saturation and levels, mapping a colour in `0.0..=1.0` onto
    /// the picture's own brightness scale.
    pub(crate) fn normalize(&self, c: [f32; 3], levels: Levels) -> [f32; 3] {
        let l = luma(c);
        let range = (levels.white - levels.black).max(1e-3);
        c.map(|v| ((l + (v - l) * self.saturation - levels.black) / range).clamp(0.0, 1.0))
    }

    /// The posterised background for a cell of mean colour `c`.
    pub(crate) fn base(&self, c: [f32; 3]) -> [f32; 3] {
        let n = self.base_levels.max(1.0);
        let l = luma(c);
        c.map(|v| {
            let v = (l + (v - l) * self.base_saturation).clamp(0.0, 1.0);
            self.base_gain * (v.powf(self.base_gamma) * n).floor() / n
        })
    }

    /// Chance that a dot of brightness `v` is lit, with the coverage
    /// threshold at `threshold`.
    pub(crate) fn dot_chance(&self, v: f32, threshold: f32) -> f32 {
        let edge = self.dot_edge.max(1e-3);
        let t = ((v - threshold + edge) / (2.0 * edge)).clamp(0.0, 1.0);
        let above = ((v - threshold) / self.dot_ramp.max(1e-3)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t) * (self.dot_min + (self.dot_max - self.dot_min) * above)
    }
}

impl Default for Look {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How bright a dot reads: the average of its luminance and strongest
/// channel, so saturated colours count but less than white.
pub(crate) fn brightness(c: [f32; 3]) -> f32 {
    (luma(c) + c[0].max(c[1]).max(c[2])) / 2.0
}

/// Rec. 601 luma of a colour in `0.0..=1.0`.
pub(crate) fn luma(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// Scales a colour's brightness.
pub(crate) fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| (f32::from(v) * k).round().clamp(0.0, 255.0) as u8)
}
