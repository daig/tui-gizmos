//! A three-line speedometer, showing either a precise readout or labelled
//! gradations.
//!
//! - **Readout:** the needle and hub live in the top two rows; the arc's 240°
//!   sweep leaves an opening at the bottom, and the readout sits in it on the
//!   last row. Text and dial share columns, so the widget is no wider than
//!   the dial.
//! - **Gradations** (no readout): a 180° dial fills all three rows, with tick
//!   values labelled just inside the arc. The needle stops short of the label
//!   it points at rather than covering it.
//!
//! The dial is an ellipse wider than it is tall (see [`MiniSpeedometer::aspect`]):
//! three rows is only 12 dots high, and stretching the arc sideways makes
//! room for text and gives the needle more travel.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Widget};

use crate::braille::BrailleGrid;
use crate::dial::{self, Fill, Geometry, Scale};
use crate::format;
use crate::macros::{dial_builder, needle_builder};

/// A compact speedometer for three terminal rows.
///
/// With a readout, 14 columns wide:
///
/// ```text
///  ⣠⡴⠶⠞⠛⠛⠛⠛⠳⠶⢦⣄
/// ⣾⠋    ⢠⡤⠤⠤⠖⠒⠉⣷
/// ⠙⠓ 49.6 MB/s⠚⠋
/// ```
///
/// With [`no_readout`](Self::no_readout), labelled gradations instead,
/// 21 columns wide:
///
/// ```text
///    ⣀⣤⡶⠶⠞⠛⠛⠛⠛⠛⠳⠶⢦⣤⣀
/// ⢀⣴⠟⠉ 100k    10M⠈⠉⠳⣄
/// ⣾⠁1k      ⣄⠤⠖⠚⠉  1G⢹⡇
/// ```
///
/// For something narrower, use a shorter readout and a rounder dial —
/// `.readout_format(format::si).aspect(1.3)` reads `49.6M` in 10 columns.
/// Taller areas scale the dial up; below three rows only the readout is
/// shown.
///
/// # Examples
///
/// ```no_run
/// use ratatui::{Frame, layout::Rect};
/// use tui_gizmos::MiniSpeedometer;
///
/// fn draw(frame: &mut Frame, area: Rect, rate: f64) {
///     frame.render_widget(MiniSpeedometer::bytes_per_sec(rate), area);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MiniSpeedometer<'a> {
    value: f64,
    min: f64,
    max: f64,
    scale: Scale,
    peak: Option<f64>,
    ticks: u16,
    sweep: Option<f64>,
    aspect: Option<f64>,
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

impl Default for MiniSpeedometer<'_> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

dial_builder!(MiniSpeedometer);
needle_builder!(MiniSpeedometer);

impl MiniSpeedometer<'_> {
    /// A linear 0–100 half dial showing `value`.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            scale: Scale::Linear,
            peak: None,
            // A tick shares cells with a 12-dot-high arc and punches a gap in
            // it, so they are off unless asked for.
            ticks: 0,
            sweep: None,
            aspect: None,
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
    /// 1 GB/s with a readout like `49.6 MB/s`. Add
    /// [`no_readout`](Self::no_readout) for gradations labelled `1k`,
    /// `100k`, `10M`, `1G` instead (`.ticks(6)` labels every decade, given
    /// four rows).
    #[must_use]
    pub fn bytes_per_sec(rate: f64) -> Self {
        Self::new(rate)
            .range(1e3, 1e9)
            .scale(Scale::Log)
            .readout_format(format::bytes_per_sec)
            .ticks(3)
            .tick_labels(format::si)
    }

    /// Sets the dial's width-to-height ratio, clamped to `1.0..=3.0`.
    /// Defaults to 2.0 with a readout (fits `49.6 MB/s` at three rows) and
    /// 1.8 without (fits four tick labels). Lower values are narrower but
    /// leave less room for text.
    #[must_use]
    pub const fn aspect(mut self, aspect: f64) -> Self {
        self.aspect = Some(aspect.clamp(1.0, 3.0));
        self
    }

    /// Fixes the arc thickness in dots (default 2).
    #[must_use]
    pub const fn thickness(mut self, dots: u16) -> Self {
        self.thickness = Some(dots);
        self
    }

    /// Sets the dial's sweep in degrees, clamped to `180..=300`. Defaults to
    /// 240 with a readout, which sits in the opening at the bottom, and 180
    /// without, which leaves the most room for tick labels.
    #[must_use]
    pub const fn sweep(mut self, degrees: f64) -> Self {
        self.sweep = Some(degrees.clamp(180.0, 300.0));
        self
    }

    fn sweep_degrees(&self) -> f64 {
        self.sweep.unwrap_or(if self.has_readout() { 240.0 } else { 180.0 })
    }

    fn aspect_ratio(&self) -> f64 {
        self.aspect.unwrap_or(if self.has_readout() { 2.0 } else { 1.8 })
    }

    /// Columns the widget occupies at `height` rows.
    #[must_use]
    pub fn dial_width(&self, height: u16) -> u16 {
        (self.radius(height) * self.aspect_ratio()).ceil() as u16
    }

    fn has_readout(&self) -> bool {
        self.readout.is_some() || self.readout_format.is_some()
    }

    /// Vertical radius in dots for a widget `height` rows tall. In both
    /// layouts the arc's top edge sits half a dot above the top row's
    /// centres, so the crown is a short flat run of dots, not a lone one.
    fn radius(&self, height: u16) -> f64 {
        if self.has_readout() {
            // Pivot at the bottom of the second-to-last row, keeping the hub
            // clear of the readout on the last row.
            f64::from(height.saturating_sub(1)) * 4.0 - 1.0
        } else {
            // The whole height: arc ends on the centres of the bottom dot row.
            let below = (-(self.sweep_degrees().to_radians() / 2.0).cos()).max(0.0);
            (f64::from(height) * 4.0 - 0.5) / (1.0 + below)
        }
    }
}

impl Widget for MiniSpeedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

impl Widget for &MiniSpeedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.render_frame(area, buf);
        if area.is_empty() {
            return;
        }

        let readout = self.readout_line();

        // Below three rows there is no room for a legible dial.
        let cols = self.dial_width(area.height).min(area.width);
        if area.height < 3 || cols < 4 {
            if let Some(line) = &readout {
                dial::draw_line(buf, area.x, area.y + area.height / 2, area.width, line);
            }
            return;
        }

        let ry = self.radius(area.height);
        let rx = f64::from(cols).min(ry * self.aspect_ratio());
        let g = Geometry {
            rx,
            ry,
            cx: rx,
            cy: ry,
            half: self.sweep_degrees().to_radians() / 2.0,
            thickness: f64::from(self.thickness.unwrap_or(2)),
        };
        let mut grid = BrailleGrid::new(cols, area.height);
        // At this size a tick mark shares cells with the arc and breaks it,
        // so labelled ticks are shown by their labels alone.
        let paint = self.paint(if self.tick_labels.is_some() { 0 } else { self.ticks });
        paint.draw_dial(&mut grid, &g);
        // Labels and a readout would crowd each other: it's one or the other.
        let labels = match self.tick_labels {
            Some(format) if self.ticks > 0 && !self.has_readout() => {
                self.place_tick_labels(area, &grid, &g, format, g.thickness + 1.0)
            }
            _ => Vec::new(),
        };
        self.finish_dial(buf, area, grid, &g, &paint, &labels);

        // The readout goes in the opening between the arc's ends.
        if let Some(line) = &readout {
            let width = (line.width() as u16).min(area.width);
            let x = (area.x + cols / 2).saturating_sub(width / 2).max(area.x);
            dial::draw_line(buf, x, area.bottom() - 1, width, line);
        }
    }
}
