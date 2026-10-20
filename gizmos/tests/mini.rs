use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_gizmos::{Fill, MiniSpeedometer, Scale};

fn text(widget: &MiniSpeedometer<'_>, width: u16, height: u16) -> Vec<String> {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    widget.render(buf.area, &mut buf);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn readout_inside_the_dial() {
    let w = MiniSpeedometer::bytes_per_sec(49.6e6);
    let cols = w.dial_width(3);
    let rows = text(&w, cols, 3);
    // The readout sits centred in the bottom opening, within the dial's width.
    assert!(rows[2].contains("49.6 MB/s"), "{rows:?}");
    let braille = |r: &str| r.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c));
    assert!(braille(&rows[0]) && braille(&rows[1]), "{rows:?}");
    // Nothing spills to the right of the dial.
    assert_eq!(
        text(&w, cols + 10, 3).iter().map(|r| r.chars().count()).max(),
        Some(cols as usize)
    );
}

#[test]
fn compact_readout_fits_a_narrower_dial() {
    let w = MiniSpeedometer::bytes_per_sec(49.6e6)
        .readout_format(tui_gizmos::format::si)
        .aspect(1.3);
    assert!(w.dial_width(3) <= 10);
    assert!(text(&w, w.dial_width(3), 3)[2].contains("49.6M"));
}

#[test]
fn single_row_is_text_only() {
    assert_eq!(text(&MiniSpeedometer::new(1.0).readout("42%"), 20, 1), ["42%"]);
}

#[test]
fn never_panics_at_any_size() {
    let minis = [
        MiniSpeedometer::new(f64::NAN).aspect(3.0).thickness(30),
        MiniSpeedometer::bytes_per_sec(1e12).peak(-1.0).ticks(4),
        MiniSpeedometer::new(5.0)
            .range(3.0, 3.0)
            .scale(Scale::Log)
            .fill(Fill::Gradient(vec![])),
    ];
    for mini in &minis {
        for w in 0..40 {
            for h in 0..8 {
                text(mini, w, h);
            }
        }
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        mini.render(Rect::new(4, 1, 30, 3), &mut buf);
    }
}

#[test]
fn formatted_readout_dims_the_unit() {
    use ratatui::style::{Color, Modifier};
    let mut buf = Buffer::empty(Rect::new(0, 0, 9, 1));
    // One row: just the readout, left-aligned.
    MiniSpeedometer::bytes_per_sec(49.6e6).render(buf.area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), "4");
    assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(5, 0)].symbol(), "M");
    assert_eq!(buf[(5, 0)].fg, Color::DarkGray);
    assert!(!buf[(5, 0)].modifier.contains(Modifier::BOLD));
}

#[test]
fn gradations_replace_the_readout() {
    let w = MiniSpeedometer::bytes_per_sec(49.6e6).no_readout();
    let rows = text(&w, w.dial_width(3), 3).join("\n");
    for label in ["1k", "100k", "10M", "1G"] {
        assert!(rows.contains(label), "missing {label}:\n{rows}");
    }
    assert!(!rows.contains("MB/s"), "{rows}");
    // With a readout, the labels stay out of its way.
    let rows = text(&MiniSpeedometer::bytes_per_sec(49.6e6), 14, 3).join("\n");
    assert!(rows.contains("49.6 MB/s") && !rows.contains("100k"), "{rows}");
}

#[test]
fn needle_never_covers_a_label() {
    // Sweep the needle across the whole dial; every label stays intact.
    for i in 0..=60 {
        let rate = 1e3 * 10f64.powf(f64::from(i) / 10.0);
        let w = MiniSpeedometer::bytes_per_sec(rate).no_readout();
        let rows = text(&w, w.dial_width(3), 3).join("\n");
        for label in ["1k", "100k", "10M", "1G"] {
            assert!(rows.contains(label), "{label} hidden at {rate}:\n{rows}");
        }
    }
}
