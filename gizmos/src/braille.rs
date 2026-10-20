//! A tiny layered braille canvas shared by the gizmos.
//!
//! Each terminal cell holds a 2×4 grid of braille dots. Because cells are
//! roughly twice as tall as they are wide, one dot is (nearly) square, so
//! geometry done in dot space looks right on screen without any aspect fix-up.
//!
//! A cell can only have one foreground colour, so dots are drawn onto
//! numbered layers. When a cell is rendered, only the highest non-empty layer
//! is shown — this keeps colour boundaries (e.g. where a filled arc meets its
//! track) accurate to the dot rather than smearing a whole cell.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

pub(crate) const LAYERS: usize = 4;

const BRAILLE_BASE: u32 = 0x2800;

/// Bit for each dot, indexed by `[dot_row % 4][dot_col % 2]`.
const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    bits: [u8; LAYERS],
    colors: [Color; LAYERS],
}

#[derive(Debug, Clone)]
pub(crate) struct BrailleGrid {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl BrailleGrid {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); usize::from(width) * usize::from(height)],
        }
    }

    pub(crate) fn dot_width(&self) -> i32 {
        i32::from(self.width) * 2
    }

    pub(crate) fn dot_height(&self) -> i32 {
        i32::from(self.height) * 4
    }

    /// Sets the dot at integer dot coordinates. Out-of-bounds dots are ignored.
    pub(crate) fn set(&mut self, x: i32, y: i32, layer: usize, color: Color) {
        if x < 0 || y < 0 || x >= self.dot_width() || y >= self.dot_height() {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let cell = &mut self.cells[(y / 4) * usize::from(self.width) + x / 2];
        cell.bits[layer] |= BRAILLE_BITS[y % 4][x % 2];
        cell.colors[layer] = color;
    }

    /// Sets the dot containing the continuous point `(x, y)`.
    pub(crate) fn plot(&mut self, x: f64, y: f64, layer: usize, color: Color) {
        self.set(x.floor() as i32, y.floor() as i32, layer, color);
    }

    /// Draws a straight line between two continuous points.
    pub(crate) fn line(&mut self, from: (f64, f64), to: (f64, f64), layer: usize, color: Color) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = (dx.abs().max(dy.abs()) * 2.0).ceil().max(1.0) as i32;
        for i in 0..=steps {
            let t = f64::from(i) / f64::from(steps);
            self.plot(from.0 + dx * t, from.1 + dy * t, layer, color);
        }
    }

    /// Whether anything was drawn in the cell at `(col, row)` on a layer
    /// below `layer`. Out-of-bounds cells are empty.
    pub(crate) fn drawn_below(&self, col: u16, row: u16, layer: usize) -> bool {
        col < self.width
            && row < self.height
            && self.cells[usize::from(row) * usize::from(self.width) + usize::from(col)].bits[..layer]
                .iter()
                .any(|&b| b != 0)
    }

    /// Writes every non-empty cell into `buf`, with `area` as the top-left origin.
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        for row in 0..self.height.min(area.height) {
            for col in 0..self.width.min(area.width) {
                let cell = &self.cells[usize::from(row) * usize::from(self.width) + usize::from(col)];
                let Some(layer) = (0..LAYERS).rev().find(|&l| cell.bits[l] != 0) else {
                    continue;
                };
                let ch = char::from_u32(BRAILLE_BASE + u32::from(cell.bits[layer])).unwrap_or(' ');
                if let Some(out) = buf.cell_mut(Position::new(area.x + col, area.y + row)) {
                    out.set_char(ch).set_fg(cell.colors[layer]);
                }
            }
        }
    }
}
