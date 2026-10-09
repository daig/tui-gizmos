//! A braille dial gauge with a needle — a drop-in for boring `49.6MB/s` text.
//!
//! ## How it works
//!
//! All geometry happens in braille *dot* space (2 dots per cell across, 4
//! down), where dots are close to square, so a circle of radius `r` dots looks
//! round on screen.
//!
//! 1. `Speedometer::layout` finds the largest radius whose dial and readout
//!    fit inside the render area.
//! 2. The dial itself is painted by [`crate::dial`]: every dot in the bounding
//!    box is tested for membership in the ring (distance) and the sweep
//!    (angle); ring dots before the value angle are *fill*, the rest *track*.
//!    Ticks, the peak marker and the needle go on higher layers of the
//!    [`BrailleGrid`] so they win colour conflicts within a cell.
//! 4. Tick labels go into cells the dial left blank, so the needle sweeps over
//!    them like a real gauge; the readout is written last.
//!
//! Angles are measured clockwise from 12 o'clock; a sweep of 240° runs from
//! −120° (about 8 o'clock) to +120° (about 4 o'clock), like a car dashboard.

use std::f64::consts::FRAC_PI_2;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Widget};

use crate::braille::BrailleGrid;
use crate::dial::{self, Fill, Geometry, Scale};
use crate::format;
use crate::macros::{dial_builder, needle_builder};

/// A dial gauge with a coloured arc, needle, tick marks and a text readout.
///
/// Like the rest of the crate, it is stateless: build it each frame from your
/// current value. Pair it with [`crate::Damped`] for a smoothly swinging
/// needle and [`crate::PeakHold`] for a decaying peak marker.
///
/// The dial scales itself to the area it is given. Roughly:
///
/// | Area (cols × rows) | Result |
/// |--------------------|--------|
/// | 12 × 4             | small dial, readout under the hub |
/// | 24 × 8             | ticks appear |
/// | 40 × 14 and up     | thick arc, tick labels |
///
/// # Examples
///
/// ```no_run
/// use ratatui::{Frame, layout::Rect};
/// use tui_gizmos::Speedometer;
///
/// fn draw(frame: &mut Frame, area: Rect, rate: f64, peak: f64) {
///     frame.render_widget(
///         Speedometer::bytes_per_sec(rate).peak(peak),
///         area,
///     );
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Speedometer<'a> {
    value: f64,
    min: f64,
    max: f64,
    scale: Scale,
    sweep: f64,
    peak: Option<f64>,
    ticks: u16,
    thickness: Option<u16>,
    fill: Fill,
    track_color: Color,
    tick_color: Color,
    needle_color: Color,
    peak_color: Color,
    readout: Option<Line<'a>>,
    readout_format: Option<fn(f64) -> String>,
    readout_style: Style,
    unit_style: Style,
    tick_labels: Option<fn(f64) -> String>,
    block: Option<Block<'a>>,
    style: Style,
}

impl Default for Speedometer<'_> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

dial_builder!(Speedometer);
needle_builder!(Speedometer);

impl Speedometer<'_> {
    /// A linear 0–100 dial showing `value`, with a 240° sweep and 5 ticks.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            scale: Scale::Linear,
            sweep: 240.0,
            peak: None,
            ticks: 5,
            thickness: None,
            fill: Fill::default(),
            track_color: Color::DarkGray,
            tick_color: Color::Gray,
            needle_color: Color::White,
            peak_color: Color::LightMagenta,
            readout: None,
            readout_format: None,
            readout_style: Style::new().bold(),
            unit_style: Style::new().dark_gray(),
            tick_labels: None,
            block: None,
            style: Style::new(),
        }
    }

    /// Preset for a transfer rate in bytes/second: a log dial from 1 kB/s to
    /// 1 GB/s with a tick per decade, readout like `49.6 MB/s`.
    #[must_use]
    pub fn bytes_per_sec(rate: f64) -> Self {
        Self::new(rate)
            .range(1e3, 1e9)
            .scale(Scale::Log)
            .ticks(6)
            .readout_format(format::bytes_per_sec)
            .tick_labels(format::si)
    }

    /// Sets the dial's sweep in degrees, clamped to `30..=330` (default 240).
    /// `180` gives a flat-bottomed half-dial, which is shorter for its width.
    #[must_use]
    pub const fn sweep(mut self, degrees: f64) -> Self {
        self.sweep = degrees.clamp(30.0, 330.0);
        self
    }

    /// Fixes the arc thickness in dots. By default it grows with the dial,
    /// from 1 to 3 dots.
    #[must_use]
    pub const fn thickness(mut self, dots: u16) -> Self {
        self.thickness = Some(dots);
        self
    }

    /// Finds the largest dial that fits `area`.
    fn layout(&self, area: Rect, text_rows: u16) -> Option<DialLayout> {
        let half = self.sweep.to_radians() / 2.0;
        let width = f64::from(area.width) * 2.0;
        let below = (-half.cos()).max(0.0);
        let reach = if half >= FRAC_PI_2 { 1.0 } else { half.sin() };
        let max_r = (width / 2.0 / reach).min(f64::from(area.height) * 4.0) as i32;

        (2..=max_r).rev().find_map(|r| {
            let rf = f64::from(r);
            let cy = rf;
            let text_row = cell_row(cy) + 1;
            let rows = (cell_row(cy + rf * below - 0.5) + 1).max(text_row + i32::from(text_rows));
            (rows <= i32::from(area.height)).then(|| DialLayout {
                g: Geometry {
                    rx: rf,
                    ry: rf,
                    cx: width / 2.0,
                    cy,
                    half,
                    thickness: f64::from(self.thickness.unwrap_or((r / 7).clamp(1, 3) as u16)),
                },
                text_row,
                rows,
            })
        })
    }
}

/// The cell row containing dot-space `y`.
fn cell_row(y: f64) -> i32 {
    (y.floor() as i32).div_euclid(4)
}

/// A dial plus where its text goes.
#[derive(Debug, Clone, Copy)]
struct DialLayout {
    g: Geometry,
    /// Cell row of the readout.
    text_row: i32,
    /// Total cell rows used.
    rows: i32,
}

impl Widget for Speedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

impl Widget for &Speedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.render_frame(area, buf);
        if area.is_empty() {
            return;
        }

        let readout = self.readout_line();
        let text_rows = u16::from(readout.is_some());
        let Some(layout) = self.layout(area, text_rows) else {
            // Too small for a dial: degrade to the plain readout.
            if let Some(line) = &readout {
                draw_centered(buf, area, 0, line);
            }
            return;
        };
        let g = layout.g;

        // Centre the dial vertically.
        let top = (i32::from(area.height) - layout.rows) / 2;
        let area = Rect {
            y: area.y + top as u16,
            height: layout.rows as u16,
            ..area
        };

        let mut grid = BrailleGrid::new(area.width, area.height);
        let show_ticks = g.ry >= 12.0 && self.ticks > 0;
        let paint = self.paint(if show_ticks { self.ticks } else { 0 });
        paint.draw_dial(&mut grid, &g);
        let labels = match self.tick_labels {
            Some(format) if show_ticks && g.ry >= 20.0 => {
                self.place_tick_labels(area, &grid, &g, format, dial::tick_inset(&g) + 2.5)
            }
            _ => Vec::new(),
        };
        self.finish_dial(buf, area, grid, &g, &paint, &labels);

        if let Some(line) = &readout {
            draw_centered(buf, area, layout.text_row, line);
        }
    }
}

fn draw_centered(buf: &mut Buffer, area: Rect, row: i32, line: &Line<'_>) {
    if row < 0 || row >= i32::from(area.height) {
        return;
    }
    let width = (line.width() as u16).min(area.width);
    dial::draw_line(buf, area.x + (area.width - width) / 2, area.y + row as u16, width, line);
}
