use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_gizmos::{Fill, Scale, Speedometer};

fn render(widget: &Speedometer<'_>, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    widget.render(buf.area, &mut buf);
    buf
}

fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn renders_readout() {
    let w = Speedometer::bytes_per_sec(49.6e6);
    let out = text(&render(&w, 30, 10));
    assert!(out.contains("49.6 MB/s"), "{out}");
}

#[test]
fn never_panics_at_any_size() {
    let dials = [
        Speedometer::new(f64::NAN),
        Speedometer::new(1e30).peak(-5.0),
        Speedometer::bytes_per_sec(0.0).peak(1e12),
        Speedometer::new(50.0)
            .sweep(180.0)
            .thickness(40)
            .fill(Fill::Gradient(vec![])),
        Speedometer::new(5.0).range(10.0, 10.0).scale(Scale::Log),
    ];
    for dial in &dials {
        for w in 0..50 {
            for h in 0..20 {
                render(dial, w, h);
            }
            // Areas hanging off the buffer edge are clipped, not a panic.
            let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
            dial.render(Rect::new(5, 2, w, 20), &mut buf);
        }
    }
}

#[test]
fn degrades_to_text_when_tiny() {
    let out = text(&render(&Speedometer::new(1.0).readout("42"), 10, 1));
    assert_eq!(out.trim(), "42");
}

#[test]
fn needle_tracks_value() {
    // At min the needle points lower-left, at max lower-right: compare which
    // half of the dial holds the needle's (white) dots.
    let side = |value| {
        let buf = render(&Speedometer::new(value).ticks(0), 20, 8);
        let white: Vec<u16> = buf
            .content
            .iter()
            .enumerate()
            .filter(|(_, c)| c.fg == ratatui::style::Color::White && c.symbol() != " ")
            .map(|(i, _)| (i % 20) as u16)
            .collect();
        white.iter().map(|&x| f64::from(x)).sum::<f64>() / white.len() as f64
    };
    assert!(side(0.0) < 9.5, "{}", side(0.0));
    assert!(side(100.0) > 9.5, "{}", side(100.0));
}

#[test]
fn fraction_mapping() {
    let lin = Speedometer::new(0.0).range(0.0, 200.0);
    assert_eq!(lin.fraction_of(50.0), 0.25);
    assert_eq!(lin.fraction_of(-1.0), 0.0);
    assert_eq!(lin.fraction_of(1e9), 1.0);
    let log = Speedometer::new(0.0).range(1e3, 1e9).scale(Scale::Log);
    assert!((log.fraction_of(1e6) - 0.5).abs() < 1e-12);
    assert_eq!(log.fraction_of(0.0), 0.0);
}

#[test]
#[ignore = "visual dump: cargo test -- --ignored --nocapture"]
fn dump() {
    for (w, h, dial) in [
        (12, 4, Speedometer::bytes_per_sec(49.6e6)),
        (20, 6, Speedometer::bytes_per_sec(49.6e6)),
        (30, 10, Speedometer::bytes_per_sec(49.6e6).peak(447e6)),
        (48, 16, Speedometer::bytes_per_sec(49.6e6).peak(447e6)),
        (30, 8, Speedometer::new(72.0).sweep(180.0).readout("72%")),
    ] {
        println!("── {w}×{h}\n{}\n", text(&render(&dial, w, h)));
    }
}

/// Writes `x\ty\tsymbol\tfg` rows for every cell of a sample scene to the path
/// in `GIZMO_DUMP`, for rasterising into a preview image.
#[test]
#[ignore = "preview dump: GIZMO_DUMP=out.tsv cargo test -- --ignored"]
fn dump_cells() {
    let Ok(path) = std::env::var("GIZMO_DUMP") else { return };
    let mut buf = Buffer::empty(Rect::new(0, 0, 112, 18));
    Speedometer::bytes_per_sec(49.6e6)
        .peak(447e6)
        .render(Rect::new(0, 0, 44, 16), &mut buf);
    Speedometer::new(72.0)
        .sweep(180.0)
        .readout("72%")
        .fill_color(ratatui::style::Color::Rgb(167, 139, 250))
        .render(Rect::new(46, 0, 30, 10), &mut buf);
    for (i, (w, h)) in [(20u16, 7u16), (16, 5), (12, 4)].into_iter().enumerate() {
        Speedometer::bytes_per_sec(2e8)
            .peak(4e8)
            .render(Rect::new(46 + [0, 22, 40][i], 10, w, h), &mut buf);
    }
    Speedometer::new(130.0)
        .range(0.0, 200.0)
        .sweep(300.0)
        .ticks(10)
        .readout("130 tok/s")
        .tick_labels(tui_gizmos::format::si)
        .render(Rect::new(78, 0, 34, 12), &mut buf);
    let rows: String = buf
        .content
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}\t{}\t{}\t{:?}\n", i % 112, i / 112, c.symbol(), c.fg))
        .collect();
    std::fs::write(path, rows).unwrap();
}

#[test]
fn labels_ticks_when_big_enough() {
    let out = text(&render(&Speedometer::bytes_per_sec(49.6e6), 48, 16));
    for label in ["1k", "10k", "100k", "1M", "10M", "100M", "1G"] {
        assert!(
            out.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == label),
            "missing {label}:\n{out}"
        );
    }
    // Small dials skip them.
    let out = text(&render(&Speedometer::bytes_per_sec(49.6e6), 20, 6));
    assert!(!out.contains("1G"), "{out}");
}
