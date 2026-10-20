//! Frames for the README images. Each test writes a frame dump that
//! `assets/render.py` turns into a PNG or GIF:
//!
//! GIZMO_RECORD=assets cargo test --test record -- --ignored
//! uv run assets/render.py assets/*.frames

use std::fmt::Write as _;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Widget;
use tui_gizmos::{ArcSpeedometer, Damped, MiniSpeedometer, PeakHold, Speedometer, format};

/// Frames per second of recorded animations.
const FPS: f64 = 25.0;

/// Appends `buf` as one frame: a `frame` line, then `x y symbol fg flags` for
/// every non-blank cell, tab-separated.
fn push_frame(out: &mut String, buf: &Buffer) {
    let w = buf.area.width;
    writeln!(out, "frame\t{}\t{}", w, buf.area.height).unwrap();
    for (i, cell) in buf.content.iter().enumerate() {
        if cell.symbol() == " " {
            continue;
        }
        let flags = [(Modifier::BOLD, 'b'), (Modifier::ITALIC, 'i')]
            .iter()
            .filter(|(m, _)| cell.modifier.contains(*m))
            .map(|(_, c)| *c)
            .collect::<String>();
        let i = i as u16;
        writeln!(out, "{}\t{}\t{}\t{:?}\t{flags}", i % w, i / w, cell.symbol(), cell.fg).unwrap();
    }
}

fn save(name: &str, frames: &str) {
    let Ok(dir) = std::env::var("GIZMO_RECORD") else { return };
    std::fs::write(format!("{dir}/{name}.frames"), frames).unwrap();
}

fn caption(text: &str, x: u16, y: u16, buf: &mut Buffer) {
    Line::from(text)
        .dark_gray()
        .render(Rect::new(x, y, text.len() as u16, 1), buf);
}

/// A download rate wandering over the whole dial: 1 kB/s to 1 GB/s and back
/// with some wobble, and a stall.
fn rate_at(t: f64) -> f64 {
    let sweep = 0.5 - 0.5 * (t * std::f64::consts::TAU / 12.0).cos();
    let wobble = (t * 2.3).sin() * 0.25 + (t * 5.9).sin() * 0.12;
    let stall = if (8.5..9.5).contains(&t) { 1e-3 } else { 1.0 };
    1.5e3 * 10f64.powf(sweep * 5.6 + wobble) * stall
}

/// Every gizmo driven by the same rate, the big dial on the left.
fn gallery(buf: &mut Buffer, rate: f64, peak: f64) {
    Speedometer::bytes_per_sec(rate)
        .peak(peak)
        .render(Rect::new(1, 0, 44, 17), buf);
    let x = 52;
    caption("MiniSpeedometer", x, 1, buf);
    MiniSpeedometer::bytes_per_sec(rate)
        .peak(peak)
        .render(Rect::new(x, 2, 14, 3), buf);
    caption(".no_readout()", x, 6, buf);
    MiniSpeedometer::bytes_per_sec(rate)
        .peak(peak)
        .no_readout()
        .render(Rect::new(x, 7, 21, 3), buf);
    caption("ArcSpeedometer", x, 11, buf);
    ArcSpeedometer::bytes_per_sec(rate)
        .peak(peak)
        .render(Rect::new(x, 12, 15, 2), buf);
}

#[test]
#[ignore = "README recording"]
fn record_gallery() {
    let mut frames = String::new();
    let dt = Duration::from_secs_f64(1.0 / FPS);
    let mut needle = Damped::new(rate_at(0.0), Duration::from_millis(250));
    let mut hold = PeakHold::new(Duration::from_secs(2), 0.6);
    for i in 0..(12.0 * FPS) as u32 {
        let rate = needle.update(rate_at(f64::from(i) / FPS), dt);
        let peak = hold.update(rate, dt);
        let mut buf = Buffer::empty(Rect::new(0, 0, 76, 17));
        gallery(&mut buf, rate, peak);
        push_frame(&mut frames, &buf);
    }
    save("gallery", &frames);
}

#[test]
#[ignore = "README recording"]
fn record_stills() {
    // Speedometer: one large labelled dial.
    let mut buf = Buffer::empty(Rect::new(0, 0, 46, 17));
    Speedometer::bytes_per_sec(49.6e6)
        .peak(2.1e8)
        .render(Rect::new(1, 0, 44, 17), &mut buf);
    let mut frames = String::new();
    push_frame(&mut frames, &buf);
    save("speedometer", &frames);

    // MiniSpeedometer: readout, gradations and compact, at a few rates.
    let mut buf = Buffer::empty(Rect::new(0, 0, 76, 12));
    for (col, rate) in [3.2e5, 4.96e7, 6.1e8].into_iter().enumerate() {
        let x = 1 + col as u16 * 26;
        MiniSpeedometer::bytes_per_sec(rate).render(Rect::new(x, 0, 14, 3), &mut buf);
        MiniSpeedometer::bytes_per_sec(rate)
            .no_readout()
            .render(Rect::new(x, 4, 21, 3), &mut buf);
        MiniSpeedometer::bytes_per_sec(rate)
            .readout_format(format::si)
            .aspect(1.3)
            .render(Rect::new(x, 8, 10, 3), &mut buf);
    }
    let mut frames = String::new();
    push_frame(&mut frames, &buf);
    save("mini", &frames);

    // ArcSpeedometer: a row of rates, then a custom colour at 3 rows.
    let mut buf = Buffer::empty(Rect::new(0, 0, 70, 6));
    for (col, rate) in [2.4e3, 3.2e5, 4.96e7, 6.1e8].into_iter().enumerate() {
        ArcSpeedometer::bytes_per_sec(rate).render(Rect::new(1 + col as u16 * 17, 0, 15, 2), &mut buf);
    }
    ArcSpeedometer::new(72.0)
        .readout("72%")
        .readout_style(Style::new().bold().magenta())
        .fill_color(ratatui::style::Color::Rgb(203, 166, 247))
        .render(Rect::new(1, 3, 22, 3), &mut buf);
    ArcSpeedometer::new(128.0)
        .range(0.0, 200.0)
        .readout("128 tok/s")
        .render(Rect::new(26, 3, 22, 3), &mut buf);
    let mut frames = String::new();
    push_frame(&mut frames, &buf);
    save("arc", &frames);
}
