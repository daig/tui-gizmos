//! Preview dumps for the mini dial: GIZMO_DUMP=out.tsv cargo test --test mini_preview -- --ignored
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_gizmos::MiniSpeedometer;

fn dump(buf: &Buffer, path: &str) {
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
#[ignore = "preview dump"]
fn dump_mini_variants() {
    let Ok(path) = std::env::var("GIZMO_DUMP") else { return };
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 20));
    let rates = [0.0, 2e3, 8e5, 4.96e7, 6e8, 2e9];
    for (i, rate) in rates.into_iter().enumerate() {
        let dial = MiniSpeedometer::bytes_per_sec(rate).peak(4.47e8);
        dial.clone().render(Rect::new(i as u16 * 16, 0, 15, 3), &mut buf);
        dial.no_readout()
            .render(Rect::new((i % 4) as u16 * 25, 4 + (i / 4) as u16 * 4, 22, 3), &mut buf);
    }
    dump(&buf, &path);
}

#[test]
#[ignore = "prints plain-text samples"]
fn print_samples() {
    for (w, h, mini) in [
        (14, 3, MiniSpeedometer::bytes_per_sec(49.6e6)),
        (
            10,
            3,
            MiniSpeedometer::bytes_per_sec(49.6e6)
                .readout_format(tui_gizmos::format::si)
                .aspect(1.3),
        ),
    ] {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        mini.render(buf.area, &mut buf);
        for y in 0..h {
            println!(
                "|{}",
                (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end()
            );
        }
        println!("|");
    }
    let mut buf = Buffer::empty(Rect::new(0, 0, 48, 16));
    tui_gizmos::Speedometer::bytes_per_sec(49.6e6).render(buf.area, &mut buf);
    for y in 0..16 {
        println!(
            "|{}",
            (0..48).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end()
        );
    }
}

#[test]
#[ignore = "prints a plain-text gradations sample"]
fn print_gradations() {
    for (w, h, mini) in [
        (21, 3, MiniSpeedometer::bytes_per_sec(49.6e6).no_readout()),
        (29, 4, MiniSpeedometer::bytes_per_sec(49.6e6).no_readout().ticks(6)),
    ] {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        mini.render(buf.area, &mut buf);
        for y in 0..h {
            println!(
                "|{}",
                (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end()
            );
        }
        println!("|");
    }
}
