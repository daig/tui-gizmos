//! The widget: a picture rendered as layered marks.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use ratatui::widgets::Widget;

use crate::blocks::{BLOCKS, Detail};
use crate::look::{Rgb, brightness, luma, scale};
use crate::noise::{hash, uniforms};
use crate::picture::{Field, box_mean, sample};
use crate::{Fit, Levels, Look, Picture};

/// Braille bit for each dot of a cell, indexed by `row * 2 + column`.
const BRAILLE: [u8; 8] = [0x01, 0x08, 0x02, 0x10, 0x04, 0x20, 0x40, 0x80];

/// Noise layers, so dots and accents draw independent numbers.
const DOT_NOISE: u32 = 0;
const MARK_NOISE: u32 = 1;

/// Renders a [`Picture`] as stipple art: a posterised dark base in the cell
/// backgrounds, with braille dots on top.
///
/// The dots are random but deterministic: the same `seed` always gives the
/// same frame. Use a new seed for each new video frame, and the same one
/// for as long as the picture doesn't change.
///
/// ```no_run
/// use ratatui::{Frame, layout::Rect};
/// use tui_stipple::{Picture, Stipple};
///
/// fn draw(frame: &mut Frame, rgb: &[u8], frame_number: u64) {
///     let picture = Picture::new(320, 240, rgb).expect("320×240 RGB");
///     frame.render_widget(Stipple::new(picture).seed(frame_number), frame.area());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Stipple<'a> {
    picture: Picture<'a>,
    look: &'a Look,
    levels: Option<Levels>,
    seed: u64,
    fit: Fit,
    detail: Detail,
    cell_aspect: f64,
}

impl<'a> Stipple<'a> {
    /// Renders `picture` with the default [`Look`].
    #[must_use]
    pub const fn new(picture: Picture<'a>) -> Self {
        Self {
            picture,
            look: &Look::DEFAULT,
            levels: None,
            seed: 0,
            fit: Fit::Contain,
            detail: Detail::Quadrants,
            cell_aspect: 0.5,
        }
    }

    /// Sets the look.
    #[must_use]
    pub const fn look(mut self, look: &'a Look) -> Self {
        self.look = look;
        self
    }

    /// Sets the exposure. Without this, each frame is measured on its own,
    /// which can pump on video; [`AutoLevels`](crate::AutoLevels) smooths it.
    #[must_use]
    pub const fn levels(mut self, levels: Levels) -> Self {
        self.levels = Some(levels);
        self
    }

    /// Sets the seed that deals the marks.
    #[must_use]
    pub const fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Sets how the picture fills the area (default [`Fit::Contain`]).
    #[must_use]
    pub const fn fit(mut self, fit: Fit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets how much detail the background carries inside each cell
    /// (default [`Detail::Quadrants`]).
    #[must_use]
    pub const fn detail(mut self, detail: Detail) -> Self {
        self.detail = detail;
        self
    }

    /// Sets a terminal cell's width over its height (default 0.5), used to
    /// keep the picture's proportions.
    #[must_use]
    pub const fn cell_aspect(mut self, aspect: f64) -> Self {
        self.cell_aspect = aspect;
        self
    }
}

impl Widget for Stipple<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

impl Widget for &Stipple<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let (cols, rows) = (usize::from(area.width), usize::from(area.height));
        let mut field = sample(&self.picture, cols * 2, rows * 4, self.fit, self.cell_aspect * 2.0);
        let levels = self.levels.unwrap_or_else(|| Levels::measure(&self.picture, self.look));
        // Picture luminance before exposure, so accents can tell dark
        // content from empty black.
        let raw: Vec<f32> = field.px.iter().map(|&c| luma(c)).collect();
        for px in &mut field.px {
            *px = self.look.normalize(*px, levels);
        }
        // Dots read a lightly blurred brightness, so they gather in regions.
        let bright: Vec<f32> = field.px.iter().map(|&c| brightness(c)).collect();
        let bright = box_mean(&bright, field.width, 1);
        // Warmth carried down each column, for drips.
        let mut trails = vec![0.0; cols];
        for row in 0..rows {
            for (col, trail) in trails.iter_mut().enumerate() {
                let planes = Planes {
                    field: &field,
                    bright: &bright,
                    raw: &raw,
                    threshold: levels.dots,
                };
                let mark = self.mark(&planes, col, row, trail);
                let pos = Position::new(area.x + col as u16, area.y + row as u16);
                if let Some(cell) = buf.cell_mut(pos) {
                    match mark.glyph {
                        Glyph::Blank => cell.set_char(' '),
                        Glyph::Braille(bits) => cell.set_char(char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')),
                        Glyph::Text(s) => cell.set_symbol(s),
                    };
                    cell.set_fg(rgb(mark.fg)).set_bg(rgb(mark.bg));
                }
            }
        }
    }
}

enum Glyph {
    Blank,
    Braille(u8),
    Text(&'static str),
}

struct Mark {
    glyph: Glyph,
    fg: Rgb,
    bg: Rgb,
}

/// Per-dot inputs to the marks, all `field.width` wide.
struct Planes<'a> {
    /// Colours after exposure.
    field: &'a Field,
    /// Lightly blurred brightness, for dots.
    bright: &'a [f32],
    /// Luminance before exposure.
    raw: &'a [f32],
    /// Brightness above which dots appear.
    threshold: f32,
}

impl Stipple<'_> {
    /// Decides one cell from its eight dots.
    fn mark(&self, planes: &Planes, col: usize, row: usize, trail: &mut f32) -> Mark {
        let look = self.look;
        let field = planes.field;
        let (x0, y0) = (col * 2, row * 4);
        let dot_at = |i: usize| (x0 + i % 2, y0 + i / 2);
        let mut dots = [None; 8];
        for (i, dot) in dots.iter_mut().enumerate() {
            let (x, y) = dot_at(i);
            *dot = field.is_inside(x, y).then(|| field.get(x, y));
        }
        if dots.iter().all(Option::is_none) {
            *trail = 0.0;
            return Mark {
                glyph: Glyph::Blank,
                fg: look.background,
                bg: look.background,
            };
        }
        let mean = dots
            .iter()
            .flatten()
            .fold([0.0; 3], |a, d| [a[0] + d[0], a[1] + d[1], a[2] + d[2]])
            .map(|v| v / 8.0);
        let split = self.split(&dots, mean);
        let base = split.bg;
        let warmth = ((mean[0] - (mean[1] + mean[2]) / 2.0) * look.drip_gain).clamp(0.0, 1.0);
        *trail = warmth.max(*trail * look.drip_decay);

        if dots.iter().all(|d| d.is_some_and(|d| luma(d) >= look.clip)) {
            let bg = look.palette[usize::from(code_of(mean))];
            return Mark {
                glyph: Glyph::Blank,
                fg: bg,
                bg,
            };
        }

        let mut bits = 0u8;
        let mut votes = [0u8; 8];
        for (i, dot) in dots.iter().enumerate() {
            let Some(d) = *dot else { continue };
            let (x, y) = dot_at(i);
            let [u0, u1, u2] = uniforms(hash(self.seed, x as u32, y as u32, DOT_NOISE));
            if u0 >= look.dot_chance(planes.bright[y * field.width + x], planes.threshold) {
                continue;
            }
            // One channel, chosen by strength…
            let w = d.map(|v| v.powf(look.hue_contrast));
            let pick = u1 * (w[0] + w[1] + w[2]);
            let c = if pick < w[0] {
                0
            } else if pick < w[0] + w[1] {
                1
            } else {
                2
            };
            // …sometimes mixed with the next strongest.
            let (a, b) = ((c + 1) % 3, (c + 2) % 3);
            let other = if w[a] >= w[b] { a } else { b };
            let mut code = 1 << c;
            if u2 < look.dot_mix * w[other] / max3(w).max(1e-6) {
                code |= 1 << other;
            }
            votes[code] += 1;
            bits |= BRAILLE[i];
        }
        if bits != 0 {
            // One colour per cell, drawn in proportion to its dots' colours,
            // so across cells the colours keep the same mix as the dots.
            let [u, _, _] = uniforms(hash(self.seed, col as u32, row as u32, MARK_NOISE));
            let total: u8 = votes.iter().sum();
            let mut pick = (u * f32::from(total)) as u8;
            let code = (1..8)
                .find(|&c| {
                    if pick < votes[c] {
                        true
                    } else {
                        pick -= votes[c];
                        false
                    }
                })
                .unwrap_or(2);
            return Mark {
                glyph: Glyph::Braille(bits),
                fg: look.palette[code],
                bg: base,
            };
        }

        let [u0, u1, u2] = uniforms(hash(self.seed, col as u32, row as u32, MARK_NOISE));
        // Dashes need some content under them, and thin out in brighter cells.
        let raw = (0..8)
            .filter(|&i| dots[i].is_some())
            .map(|i| {
                let (x, y) = dot_at(i);
                planes.raw[y * field.width + x]
            })
            .sum::<f32>()
            / 8.0;
        let content = ((raw - look.dash_floor) / look.dash_floor.max(1e-3)).clamp(0.0, 1.0);
        let l = luma(mean);
        let fade = if l <= look.dash_max {
            1.0
        } else {
            (2.0 - l / look.dash_max).max(0.0)
        };
        let dash = content * fade;
        if !look.dash_glyphs.is_empty() && u0 < look.dash_density * dash {
            let i = (u1 * look.dash_glyphs.len() as f32) as usize;
            return Mark {
                glyph: Glyph::Text(look.dash_glyphs[i.min(look.dash_glyphs.len() - 1)]),
                fg: look.dash_color,
                bg: base,
            };
        }
        if u2 < look.drip_density * *trail {
            return Mark {
                glyph: Glyph::Text(look.drip_glyph),
                fg: scale(look.drip_color, 0.35 + 0.65 * trail.min(1.0)),
                bg: base,
            };
        }
        Mark {
            glyph: if split.bits == 0 {
                Glyph::Blank
            } else {
                Glyph::Text(BLOCKS[split.bits])
            },
            fg: split.fg,
            bg: split.bg,
        }
    }

    /// The cell's background: two posterised colours split at the
    /// resolution `detail` allows. Cells with marks use only the darker
    /// side, so a cell's background doesn't flicker as marks come and go.
    fn split(&self, dots: &[Option<[f32; 3]>; 8], mean: [f32; 3]) -> Split {
        let flat = to_rgb(self.look.base(mean));
        let even = Split {
            bits: 0,
            fg: flat,
            bg: flat,
        };
        let groups = self.detail.groups();
        if groups.len() == 1 {
            return even;
        }
        // Dots outside the picture count as black.
        let mean_of = |indices: &mut dyn Iterator<Item = usize>| {
            let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
            for i in indices {
                let d = dots[i].unwrap_or_default();
                (0..3).for_each(|c| sum[c] += d[c]);
                n += 1.0;
            }
            sum.map(|v| v / n.max(1.0))
        };
        let lumas: Vec<f32> = groups
            .iter()
            .map(|g| luma(self.look.base(mean_of(&mut g.iter().copied()))))
            .collect();
        let (lo, hi) = lumas
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &l| (lo.min(l), hi.max(l)));
        if hi - lo < 1e-6 {
            return even;
        }
        // Split at the midpoint: the brighter blocks take the foreground.
        let threshold = (lo + hi) / 2.0;
        let bright = |k: usize| lumas[k] > threshold;
        let bits = (0..groups.len())
            .filter(|&k| bright(k))
            .flat_map(|k| groups[k].iter())
            .fold(0usize, |bits, &i| bits | 1 << i);
        let side = |want: bool| {
            to_rgb(
                self.look.base(mean_of(
                    &mut (0..groups.len())
                        .filter(|&k| bright(k) == want)
                        .flat_map(|k| groups[k].iter().copied()),
                )),
            )
        };
        let (fg, bg) = (side(true), side(false));
        if fg == bg { even } else { Split { bits, fg, bg } }
    }
}

/// A cell background in two colours: `fg` fills the sub-blocks in `bits`.
struct Split {
    bits: usize,
    fg: Rgb,
    bg: Rgb,
}

fn max3(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2])
}

/// Palette index of a colour: the channels within reach of its strongest.
fn code_of(c: [f32; 3]) -> u8 {
    let m = max3(c);
    (0..3)
        .filter(|&i| c[i] >= 0.6 * m)
        .fold(0, |code, i| code | 1 << i)
        .max(1)
}

fn to_rgb(c: [f32; 3]) -> Rgb {
    c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
}

const fn rgb(c: Rgb) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}
