//! Dial drawing shared by the speedometer gizmos.
//!
//! Geometry is in braille dot space, where dots are close to square. A dial
//! is an arc of an ellipse with horizontal radius `rx` and vertical radius
//! `ry` (equal for a round dial). Angles are measured clockwise from 12
//! o'clock and are *parametric*: the point at angle `a` is
//! `(cx + rx·sin a, cy − ry·cos a)`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::braille::BrailleGrid;

const TRACK: usize = 0;
const FILL: usize = 1;
const MARK: usize = 2;
pub(crate) const NEEDLE: usize = 3;

/// How values map onto the dial's sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    /// Evenly spaced between `min` and `max`.
    #[default]
    Linear,
    /// Logarithmic between `min` and `max` — each tick is a power of
    /// `(max/min)^(1/ticks)`. Ideal for network rates, which span orders of
    /// magnitude. `min` must be positive; non-positive values are pinned to
    /// the start of the dial.
    Log,
}

impl Scale {
    /// The value at `fraction` (0.0..=1.0) of the way from `min` to `max`.
    #[must_use]
    pub fn value_at(self, fraction: f64, min: f64, max: f64) -> f64 {
        match self {
            Self::Linear => min + fraction * (max - min),
            Self::Log => min * (max / min).powf(fraction),
        }
    }

    /// Where `value` falls between `min` and `max`, from 0.0 to 1.0.
    #[must_use]
    pub fn fraction(self, value: f64, min: f64, max: f64) -> f64 {
        let t = match self {
            Self::Linear => (value - min) / (max - min),
            Self::Log if value <= 0.0 || min <= 0.0 => 0.0,
            Self::Log => (value / min).ln() / (max / min).ln(),
        };
        if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) }
    }
}

/// How the filled part of the arc is coloured.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// One colour for the whole filled arc.
    Solid(Color),
    /// Colour stops along the dial as `(position 0.0..=1.0, colour)`, sorted
    /// by position. RGB stops are blended smoothly; other colours step.
    Gradient(Vec<(f64, Color)>),
}

impl Default for Fill {
    /// Cool-to-hot: cyan → green → yellow → red.
    fn default() -> Self {
        Self::Gradient(vec![
            (0.0, Color::Rgb(56, 189, 248)),
            (0.4, Color::Rgb(74, 222, 128)),
            (0.75, Color::Rgb(250, 204, 21)),
            (1.0, Color::Rgb(248, 113, 113)),
        ])
    }
}

impl Fill {
    /// The colour at position `t` (0.0 = dial start, 1.0 = dial end).
    #[must_use]
    pub fn color_at(&self, t: f64) -> Color {
        let stops = match self {
            Self::Solid(color) => return *color,
            Self::Gradient(stops) => stops,
        };
        let Some(&(_, first)) = stops.first() else {
            return Color::Reset;
        };
        let mut prev = (f64::NEG_INFINITY, first);
        for &(pos, color) in stops {
            if t <= pos {
                return lerp_color(prev.1, color, (t - prev.0) / (pos - prev.0));
            }
            prev = (pos, color);
        }
        prev.1
    }
}

fn lerp_color(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) if t.is_finite() => {
            let mix = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
            Color::Rgb(mix(r0, r1), mix(g0, g1), mix(b0, b1))
        }
        _ if t < 0.5 || !t.is_finite() => a,
        _ => b,
    }
}

/// Where a dial sits in its grid, in dots.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry {
    /// Outer radii of the arc.
    pub rx: f64,
    pub ry: f64,
    /// Pivot.
    pub cx: f64,
    pub cy: f64,
    /// Half the sweep, in radians.
    pub half: f64,
    pub thickness: f64,
}

impl Geometry {
    /// The point at `angle`, `inset` dots inside the outer edge.
    pub fn point(&self, inset: f64, angle: f64) -> (f64, f64) {
        (
            self.cx + (self.rx - inset) * angle.sin(),
            self.cy - (self.ry - inset) * angle.cos(),
        )
    }

    /// The angle at `fraction` (0.0..=1.0) of the sweep.
    pub fn angle_at(&self, fraction: f64) -> f64 {
        -self.half + fraction * self.half * 2.0
    }
}

/// What to draw on a dial. Positions are fractions of the sweep.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Paint<'a> {
    pub value: f64,
    pub peak: Option<f64>,
    /// Tick intervals; 0 draws none.
    pub ticks: u16,
    pub fill: &'a Fill,
    pub track_color: Color,
    pub tick_color: Color,
    pub needle_color: Color,
    pub peak_color: Color,
}

impl Paint<'_> {
    /// Draws the arc and tick marks. The needle is separate so labels can be
    /// placed in between.
    pub fn draw_dial(&self, grid: &mut BrailleGrid, g: &Geometry) {
        self.arc(grid, g);
        self.ticks(grid, g);
    }

    /// Ring dots before the value are fill, the rest track.
    fn arc(&self, grid: &mut BrailleGrid, g: &Geometry) {
        let value_angle = g.angle_at(self.value);
        let peak_angle = self.peak.map(|p| g.angle_at(p));
        let sweep = g.half * 2.0;

        for y in 0..grid.dot_height() {
            for x in 0..grid.dot_width() {
                let (px, py) = (f64::from(x) + 0.5 - g.cx, f64::from(y) + 0.5 - g.cy);
                // Normalised radius, and a first-order distance to the outer
                // edge that stays even around an ellipse (exact for a circle).
                let (nx, ny) = (px / g.rx, py / g.ry);
                let f = nx.hypot(ny);
                if f < 0.25 {
                    continue;
                }
                let grad = (px / (g.rx * g.rx)).hypot(py / (g.ry * g.ry)) / f;
                let dist = (f - 1.0) / grad;
                if dist > 0.0 || dist <= -g.thickness {
                    continue;
                }
                let angle = nx.atan2(-ny);
                if angle.abs() > g.half + 1e-9 {
                    continue;
                }
                // Arc length per radian at this point, to size the peak mark.
                let stride = f * (g.rx * angle.cos()).hypot(g.ry * angle.sin());
                if peak_angle.is_some_and(|p| ((angle - p) * stride).abs() <= 0.75) {
                    grid.set(x, y, MARK, self.peak_color);
                } else if angle <= value_angle && self.value > 0.0 {
                    grid.set(x, y, FILL, self.fill.color_at((angle + g.half) / sweep));
                } else {
                    grid.set(x, y, TRACK, self.track_color);
                }
            }
        }
    }

    fn ticks(&self, grid: &mut BrailleGrid, g: &Geometry) {
        if self.ticks == 0 {
            return;
        }
        let inset = g.thickness + 1.0;
        let len = tick_inset(g) - inset + 1.0;
        for i in 0..=self.ticks {
            let angle = g.angle_at(f64::from(i) / f64::from(self.ticks));
            grid.line(
                g.point(inset, angle),
                g.point(inset + len - 1.0, angle),
                MARK,
                self.tick_color,
            );
        }
    }

    /// Draws the needle from the hub outward, stopping at the first cell that
    /// already holds part of the dial or that `blocked` reserves (for a
    /// label, so the needle points at it rather than covering it).
    pub fn draw_needle(&self, grid: &mut BrailleGrid, g: &Geometry, blocked: impl Fn(u16, u16) -> bool) {
        let gap = if self.ticks > 0 { 2.0 } else { 1.0 };
        let tip = g.point(g.thickness + gap, g.angle_at(self.value));
        // A cell shows only its top layer, so a needle tip sharing a cell with
        // the ring would punch a hole in it.
        let (dx, dy) = (tip.0 - g.cx, tip.1 - g.cy);
        let steps = (dx.abs().max(dy.abs()) * 2.0).ceil().max(1.0) as i32;
        for i in 0..=steps {
            let t = f64::from(i) / f64::from(steps);
            let (x, y) = ((g.cx + dx * t).floor(), (g.cy + dy * t).floor());
            let (col, row) = ((x / 2.0) as u16, (y / 4.0) as u16);
            if x >= 0.0 && y >= 0.0 && (grid.drawn_below(col, row, NEEDLE) || blocked(col, row)) {
                break;
            }
            grid.plot(x, y, NEEDLE, self.needle_color);
        }
        // Hub: every dot centred within about a dot of the pivot.
        let (x0, y0) = (g.cx.floor() as i32, g.cy.floor() as i32);
        for y in y0 - 2..=y0 + 2 {
            for x in x0 - 2..=x0 + 2 {
                let (dx, dy) = (f64::from(x) + 0.5 - g.cx, f64::from(y) + 0.5 - g.cy);
                if dx.hypot(dy) <= 1.15 {
                    grid.set(x, y, NEEDLE, self.needle_color);
                }
            }
        }
    }
}

/// How far inside the outer edge the ticks reach, in dots.
pub(crate) fn tick_inset(g: &Geometry) -> f64 {
    g.thickness + (g.ry.min(g.rx) / 10.0).clamp(1.0, 3.0)
}

/// Writes `line` at `(x, y)`, clipped to `max_width` cells.
///
/// The rect is sized to the text because a [`Line`] styles its whole area,
/// which would recolour neighbouring dial cells. The cells are reset first so
/// a needle passing underneath can't garble the text.
pub(crate) fn draw_line(buf: &mut Buffer, x: u16, y: u16, max_width: u16, line: &Line<'_>) {
    let width = (line.width() as u16).min(max_width);
    if width == 0 {
        return;
    }
    let area = Rect::new(x, y, width, 1).intersection(buf.area);
    buf.set_string(area.x, area.y, " ".repeat(usize::from(area.width)), Style::reset());
    line.render(area, buf);
}
