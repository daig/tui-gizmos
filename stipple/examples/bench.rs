//! Times rendering one frame at typical and large terminal sizes.
//!
//! cargo run -p tui-stipple --release --example bench

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_stipple::{Detail, Levels, Look, Picture, Stipple};

fn main() {
    let (w, h) = (1000, 560);
    // A colourful pattern with structure at every scale.
    let rgb: Vec<u8> = (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
                let v = ((fx * 40.0).sin() * (fy * 30.0).cos() * 0.5 + 0.5) * 255.0;
                [v as u8, (fx * 255.0) as u8, (fy * 255.0) as u8]
            })
        })
        .collect();
    let picture = Picture::new(w, h, &rgb).expect("w × h RGB");
    let levels = Levels::measure(&picture, &Look::DEFAULT);
    for (cols, rows) in [(120, 40), (250, 70)] {
        let area = Rect::new(0, 0, cols, rows);
        let mut buf = Buffer::empty(area);
        let n = 50;
        let start = Instant::now();
        for seed in 0..n {
            Stipple::new(picture)
                .levels(levels)
                .detail(Detail::Octants)
                .seed(seed)
                .render(area, &mut buf);
        }
        println!("{cols}×{rows}: {:?} per frame", start.elapsed() / n as u32);
    }
}
