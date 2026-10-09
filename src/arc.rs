//! A needle-less speedometer: a lit arc with the readout inside it.
//!
//! Without a needle and hub, the inside of the arc is free for text, so the
//! whole gizmo fits in two rows: the arc's crown takes the top row, and the
//! readout sits centred on the bottom row between the arc's sides. The value
//! is shown by how far the arc is lit.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Widget};

use crate::braille::BrailleGrid;
use crate::dial::{self, Fill, Geometry, Paint, Scale};
use crate::format;
use crate::macros::dial_builder;

/// A two-row arc gauge with the readout centred inside the arc, 15 columns
/// wide.
///
/// ```text
///  ⣀⣤⠶⠶⠛⠛⠛⠛⠛⠶⠶⡤⣀
/// ⣼⠋ 49.6 MB/s ⠙⣧
/// ```
///
/// Taller areas scale the arc up, keeping the readout on the bottom row; at
/// one row only the readout is shown.
///
/// # Examples
///
/// ```no_run
/// use ratatui::{Frame, layout::Rect};
/// use tui_gizmos::ArcSpeedometer;
///
/// fn draw(frame: &mut Frame, area: Rect, rate: f64) {
///     frame.render_widget(ArcSpeedometer::bytes_per_sec(rate), area);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ArcSpeedometer<'a> {
    value: f64,
    min: f64,
    max: f64,
    scale: Scale,
    peak: Option<f64>,
    aspect: f64,
    thickness: Option<u16>,
    fill: Fill,
    track_color: Color,
    peak_color: Color,
    readout: Option<Line<'a>>,
    readout_format: Option<fn(f64) -> String>,
    readout_style: Style,
    unit_style: Style,
    block: Option<Block<'a>>,
    style: Style,
}

impl Default for ArcSpeedometer<'_> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

dial_builder!(ArcSpeedometer);

impl ArcSpeedometer<'_> {
    /// A linear 0–100 arc showing `value`.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            scale: Scale::Linear,
            peak: None,
            aspect: 2.0,
            thickness: None,
            fill: Fill::default(),
            track_color: Color::DarkGray,
            peak_color: Color::LightMagenta,
            readout: None,
            readout_format: None,
            readout_style: Style::new().bold(),
            unit_style: Style::new().dark_gray(),
            block: None,
            style: Style::new(),
        }
    }

    /// Preset for a transfer rate in bytes/second: a log arc from 1 kB/s to
    /// 1 GB/s with a readout like `49.6 MB/s`.
    #[must_use]
    pub fn bytes_per_sec(rate: f64) -> Self {
        Self::new(rate)
            .range(1e3, 1e9)
            .scale(Scale::Log)
            .readout_format(format::bytes_per_sec)
    }

    /// Sets the arc's width-to-height ratio, clamped to `1.0..=4.0` (default
    /// 2.0, which fits a `49.6 MB/s` readout at two rows).
    #[must_use]
    pub const fn aspect(mut self, aspect: f64) -> Self {
        self.aspect = aspect.clamp(1.0, 4.0);
        self
    }

    /// Fixes the arc thickness in dots (default 2).
    #[must_use]
    pub const fn thickness(mut self, dots: u16) -> Self {
        self.thickness = Some(dots);
        self
    }

    /// Columns the widget occupies at `height` rows.
    #[must_use]
    pub fn width(&self, height: u16) -> u16 {
        (radius(height) * self.aspect).ceil() as u16
    }
}

/// Vertical radius in dots for an arc `height` rows tall: the ends sit on the
/// centres of the bottom dot row and the top edge half a dot above the top
/// row's centres, so the crown is a short flat run rather than a lone dot.
fn radius(height: u16) -> f64 {
    f64::from(height) * 4.0 - 0.5
}

impl Widget for ArcSpeedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}

impl Widget for &ArcSpeedometer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.render_frame(area, buf);
        if area.is_empty() {
            return;
        }
        let readout = self.readout_line();

        let cols = self.width(area.height).min(area.width);
        if area.height >= 2 && cols >= 4 {
            let ry = radius(area.height);
            let rx = f64::from(cols).min(ry * self.aspect);
            let g = Geometry {
                rx,
                ry,
                cx: rx,
                cy: ry,
                half: std::f64::consts::FRAC_PI_2,
                thickness: f64::from(self.thickness.unwrap_or(2)),
            };
            let mut grid = BrailleGrid::new(cols, area.height);
            Paint {
                value: self.fraction_of(self.value),
                peak: self.peak.map(|p| self.fraction_of(p)),
                ticks: 0,
                fill: &self.fill,
                track_color: self.track_color,
                tick_color: Color::Reset,
                needle_color: Color::Reset,
                peak_color: self.peak_color,
            }
            .draw_dial(&mut grid, &g);
            grid.render(area, buf);
        }

        if let Some(line) = &readout {
            let width = (line.width() as u16).min(area.width);
            let x = area.x + (cols.max(width) - width) / 2;
            dial::draw_line(buf, x, area.bottom() - 1, width, line);
        }
    }
}
