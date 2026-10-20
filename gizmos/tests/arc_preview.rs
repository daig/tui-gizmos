//! Preview dumps for the arc gauge: GIZMO_DUMP=out.tsv cargo test --test arc_preview -- --ignored
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tui_gizmos::ArcSpeedometer;

#[test]
#[ignore = "preview dump"]
fn dump_arcs() {
    let Ok(path) = std::env::var("GIZMO_DUMP") else { return };
    let mut buf = Buffer::empty(Rect::new(0, 0, 112, 20));
    let rates = [0.0, 2e3, 8e5, 4.96e7, 6e8, 2e9];
    let variants: [fn(f64) -> ArcSpeedometer<'static>; 4] = [
        |r| ArcSpeedometer::bytes_per_sec(r).peak(4.47e8),
        |r| ArcSpeedometer::bytes_per_sec(r).aspect(1.8),
        |r| ArcSpeedometer::bytes_per_sec(r).thickness(1),
        |r| ArcSpeedometer::bytes_per_sec(r).aspect(2.4).thickness(3),
    ];
    for (row, make) in variants.iter().enumerate() {
        for (i, rate) in rates.into_iter().enumerate() {
            make(rate).render(Rect::new(i as u16 * 18, row as u16 * 3, 17, 2), &mut buf);
        }
    }
    for (i, rate) in [8e5, 4.96e7, 6e8].into_iter().enumerate() {
        ArcSpeedometer::bytes_per_sec(rate).render(Rect::new(i as u16 * 24, 12, 22, 3), &mut buf);
    }
    let w = buf.area.width as usize;
    let rows: String = buf
        .content
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}\t{}\t{}\t{:?}\n", i % w, i / w, c.symbol(), c.fg))
        .collect();
    std::fs::write(path, rows).unwrap();
}

#[test]
#[ignore = "prints a plain-text sample"]
fn print_arc() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 2));
    ArcSpeedometer::bytes_per_sec(49.6e6).render(buf.area, &mut buf);
    for y in 0..2 {
        println!(
            "|{}",
            (0..15).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end()
        );
    }
}
