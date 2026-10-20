//! Sporadic datamosh-style glitches drawn over a rendered area.

use std::time::Duration;

use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use crate::Rgb;

/// Short-lived glitches over whatever was rendered into an area: bright
/// bars, smeared diagonal streaks, torn rows and frozen blocks.
///
/// Keep one in your app state and apply it after rendering each frame:
///
/// ```no_run
/// use std::time::Duration;
/// use ratatui::Frame;
/// use tui_stipple::Glitch;
///
/// fn draw(frame: &mut Frame, glitch: &mut Glitch, dt: Duration) {
///     // … render a `Stipple` into frame.area() …
///     glitch.apply(frame.area(), frame.buffer_mut(), dt);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Glitch {
    rng: u64,
    rate: f32,
    color: Rgb,
    events: Vec<Event>,
}

#[derive(Debug, Clone)]
struct Event {
    kind: Kind,
    /// Cells affected, relative to the area.
    rect: Rect,
    left: Duration,
    /// Keeps the glitch's own randomness the same for its whole life.
    seed: u64,
}

#[derive(Debug, Clone)]
enum Kind {
    /// A ragged bright horizontal bar.
    Bar,
    /// Diagonal streaks, `slope` columns per row.
    Smear { slope: i16 },
    /// Rows shifted sideways by `shift` columns.
    Tear { shift: i16 },
    /// A block frozen at what it showed when the glitch began.
    Hold { cells: Vec<Cell> },
}

/// Expected new glitches per second at rate 1, and how long each lasts.
const BAR: (f32, f32, f32) = (3.0, 0.06, 0.15);
const SMEAR: (f32, f32, f32) = (0.6, 0.1, 0.3);
const TEAR: (f32, f32, f32) = (0.8, 0.06, 0.15);
const HOLD: (f32, f32, f32) = (0.5, 0.2, 0.6);

impl Glitch {
    /// Glitches dealt by `seed`, at rate 1 in bright green.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            rng: seed | 1,
            rate: 1.0,
            color: [40, 245, 50],
            events: Vec::new(),
        }
    }

    /// Scales how often glitches start (default 1; 0 stops new ones).
    #[must_use]
    pub const fn rate(mut self, rate: f32) -> Self {
        self.rate = rate;
        self
    }

    /// Sets the colour of bars and smears.
    #[must_use]
    pub const fn color(mut self, color: Rgb) -> Self {
        self.color = color;
        self
    }

    /// Advances by `dt`, starting and ending glitches, then draws the live
    /// ones over `area`.
    pub fn apply(&mut self, area: Rect, buf: &mut Buffer, dt: Duration) {
        let area = area.intersection(buf.area);
        self.events.retain_mut(|e| {
            e.left = e.left.saturating_sub(dt);
            !e.left.is_zero()
        });
        if area.is_empty() {
            return;
        }
        let secs = dt.as_secs_f32().min(0.25);
        for (kind, (per_sec, min, max)) in [(0, BAR), (1, SMEAR), (2, TEAR), (3, HOLD)] {
            if self.unit() < per_sec * self.rate * secs {
                let left = Duration::from_secs_f32(min + (max - min) * self.unit());
                let event = self.start(kind, area, buf, left);
                self.events.push(event);
            }
        }
        for event in &self.events {
            self.draw(event, area, buf);
        }
    }

    fn start(&mut self, kind: u8, area: Rect, buf: &Buffer, left: Duration) -> Event {
        let (w, h) = (area.width, area.height);
        let span = |g: &mut Self, lo: f32, hi: f32, of: u16| {
            ((f32::from(of) * (lo + (hi - lo) * g.unit())).round() as u16).clamp(1, of)
        };
        let place = |g: &mut Self, size: u16, of: u16| (g.unit() * f32::from(of - size + 1)) as u16;
        let (kind, rect) = match kind {
            0 => {
                // Bars cluster near the top, like a corrupted first slice.
                let bw = span(self, 0.15, 0.6, w);
                let y = (f32::from(h) * 0.35 * self.unit() * self.unit()) as u16;
                let x = place(self, bw, w);
                (Kind::Bar, Rect::new(x, y.min(h - 1), bw, 1))
            }
            1 => {
                let (sw, sh) = (span(self, 0.08, 0.35, w), span(self, 0.1, 0.45, h));
                let rect = Rect::new(place(self, sw, w), place(self, sh, h), sw, sh);
                let slope = if self.unit() < 0.5 { 1 } else { -1 };
                (Kind::Smear { slope }, rect)
            }
            2 => {
                let th = span(self, 0.02, 0.12, h);
                let shift = (span(self, 0.02, 0.15, w) as i16) * if self.unit() < 0.5 { 1 } else { -1 };
                (Kind::Tear { shift }, Rect::new(0, place(self, th, h), w, th))
            }
            _ => {
                let (hw, hh) = (span(self, 0.1, 0.4, w), span(self, 0.1, 0.4, h));
                let rect = Rect::new(place(self, hw, w), place(self, hh, h), hw, hh);
                let cells = cells_of(buf, offset(rect, area));
                (Kind::Hold { cells }, rect)
            }
        };
        let seed = self.rng;
        Event { kind, rect, left, seed }
    }

    fn draw(&self, event: &Event, area: Rect, buf: &mut Buffer) {
        let rect = offset(event.rect, area).intersection(area);
        let color = Color::Rgb(self.color[0], self.color[1], self.color[2]);
        match &event.kind {
            Kind::Bar => {
                // Mostly thin, sometimes full height, with frayed ends.
                let len = u32::from(rect.width);
                for (i, pos) in rect.positions().enumerate() {
                    let r = crate::noise::hash(event.seed, i as u32, 0, 0);
                    let end = (i as u32).min(len - 1 - i as u32);
                    if end < 3 && r.is_multiple_of(3) {
                        continue;
                    }
                    let glyph = ["▀", "▀", "▀", "█", "█", "▄"][(r >> 8) as usize % 6];
                    let shade = if (r >> 16).is_multiple_of(4) {
                        Color::Rgb(0, 150, 20)
                    } else {
                        color
                    };
                    if let Some(cell) = buf.cell_mut(pos) {
                        cell.set_symbol(glyph).set_fg(shade);
                    }
                }
            }
            Kind::Smear { slope } => {
                // Rows slide sideways as they go down, with diagonal streaks
                // drawn in braille so they read as dragged pixels.
                for (i, row) in (rect.top()..rect.bottom()).enumerate() {
                    let shift = i as i32 * i32::from(*slope);
                    for x in rect.left()..rect.right() {
                        let col = i32::from(x) + shift;
                        let Ok(col) = u16::try_from(col) else { continue };
                        if !area.contains(Position::new(col, row)) {
                            continue;
                        }
                        // Dots on the lines x + y ≡ 0 (mod 3), in dot coordinates.
                        let phase = (u32::from(col) * 2 + i as u32 * 4) % 3;
                        let bits = DIAGONALS[phase as usize];
                        if let Some(cell) = buf.cell_mut((col, row)) {
                            cell.set_char(char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' '))
                                .set_fg(color)
                                .set_bg(Color::Rgb(0, 70, 10));
                        }
                    }
                }
            }
            Kind::Tear { shift } => {
                for row in rect.top()..rect.bottom() {
                    let line: Vec<Cell> = (area.left()..area.right()).map(|x| buf[(x, row)].clone()).collect();
                    let n = line.len() as i32;
                    for (i, x) in (area.left()..area.right()).enumerate() {
                        let from = (i as i32 - i32::from(*shift)).rem_euclid(n) as usize;
                        buf[(x, row)] = line[from].clone();
                    }
                }
            }
            Kind::Hold { cells } => {
                let full = offset(event.rect, area);
                for (i, pos) in full.positions().enumerate() {
                    if area.contains(pos)
                        && let (Some(cell), Some(held)) = (buf.cell_mut(pos), cells.get(i))
                    {
                        *cell = held.clone();
                    }
                }
            }
        }
    }

    /// A uniform value in `[0, 1)` (xorshift64*).
    fn unit(&mut self) -> f32 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }
}

impl Default for Glitch {
    fn default() -> Self {
        Self::new(0x9E37_79B9_7F4A_7C15)
    }
}

/// Braille patterns lighting the dots where `x + y ≡ 0 (mod 3)`, for a cell
/// whose top-left dot has `x + y ≡ phase`.
const DIAGONALS: [u8; 3] = [0x61, 0x14, 0x8A];

fn offset(rect: Rect, area: Rect) -> Rect {
    Rect::new(area.x + rect.x, area.y + rect.y, rect.width, rect.height)
}

fn cells_of(buf: &Buffer, rect: Rect) -> Vec<Cell> {
    rect.positions()
        .map(|pos| buf.cell(pos).cloned().unwrap_or_default())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        for (i, cell) in buf.content.iter_mut().enumerate() {
            cell.set_char(char::from(b'a' + (i % 26) as u8));
        }
        buf
    }

    #[test]
    fn rate_zero_changes_nothing() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = filled(area);
        let before = buf.clone();
        let mut glitch = Glitch::new(1).rate(0.0);
        for _ in 0..100 {
            glitch.apply(area, &mut buf, Duration::from_millis(60));
        }
        assert_eq!(buf, before);
    }

    #[test]
    fn glitches_come_and_go() {
        let area = Rect::new(0, 0, 60, 30);
        let mut glitch = Glitch::new(7);
        let (mut touched, mut clean) = (0, 0);
        for _ in 0..200 {
            let mut buf = filled(area);
            let before = buf.clone();
            glitch.apply(area, &mut buf, Duration::from_millis(60));
            if buf == before {
                clean += 1;
            } else {
                touched += 1;
            }
        }
        assert!(touched > 20 && clean > 20, "touched {touched}, clean {clean}");
    }

    #[test]
    fn stays_inside_the_area_at_any_size() {
        for (w, h) in [(1, 1), (2, 1), (1, 9), (7, 3), (80, 2)] {
            let outer = Rect::new(0, 0, w + 4, h + 4);
            let area = Rect::new(2, 2, w, h);
            let mut glitch = Glitch::new(u64::from(w * 31 + h));
            for _ in 0..300 {
                let mut buf = filled(outer);
                let before = buf.clone();
                glitch.apply(area, &mut buf, Duration::from_millis(80));
                for pos in outer.positions() {
                    if !area.contains(pos) {
                        assert_eq!(buf[pos], before[pos], "{pos:?} outside {area:?}");
                    }
                }
            }
        }
    }
}
