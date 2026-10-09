use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_gizmos::{ArcSpeedometer, Scale};

fn text(widget: &ArcSpeedometer<'_>, width: u16, height: u16) -> Vec<String> {
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

fn is_braille(c: char) -> bool {
    ('\u{2800}'..='\u{28ff}').contains(&c)
}

#[test]
fn readout_sits_inside_the_arc_in_two_rows() {
    let w = ArcSpeedometer::bytes_per_sec(49.6e6);
    assert_eq!(w.width(2), 15);
    let rows = text(&w, 15, 2);
    assert!(rows[0].chars().all(|c| is_braille(c) || c == ' '), "{rows:?}");
    // Arc on both sides of the readout, which is centred.
    let bottom: Vec<char> = rows[1].chars().collect();
    assert!(is_braille(bottom[0]) && is_braille(*bottom.last().unwrap()), "{rows:?}");
    assert_eq!(rows[1].trim_matches(is_braille).trim(), "49.6 MB/s");
}

#[test]
fn every_byte_rate_fits_between_the_sides() {
    // The widest readouts must not cut into the arc.
    for rate in [0.0, 999.0, 2e3, 49.6e6, 999e6, 2e9] {
        let rows = text(&ArcSpeedometer::bytes_per_sec(rate), 15, 2);
        let bottom: Vec<char> = rows[1].chars().collect();
        assert!(is_braille(bottom[0]) && is_braille(bottom[14]), "{rate}: {rows:?}");
    }
}

#[test]
fn single_row_is_text_only() {
    let rows = text(&ArcSpeedometer::new(1.0).readout("42%"), 10, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].trim(), "42%");
}

#[test]
fn never_panics_at_any_size() {
    let arcs = [
        ArcSpeedometer::new(f64::NAN).aspect(4.0).thickness(40),
        ArcSpeedometer::bytes_per_sec(1e12).peak(-1.0).no_readout(),
        ArcSpeedometer::new(5.0)
            .range(3.0, 3.0)
            .scale(Scale::Log)
            .readout("a very long readout"),
    ];
    for arc in &arcs {
        for w in 0..40 {
            for h in 0..8 {
                text(arc, w, h);
            }
        }
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        arc.render(Rect::new(4, 1, 30, 3), &mut buf);
    }
}
