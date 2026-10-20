use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use tui_stipple::{Detail, Fit, Levels, Look, Picture, Stipple};

fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
    rgb.repeat(w * h)
}

/// A horizontal ramp from black on the left to `to` on the right.
fn ramp(w: usize, h: usize, to: [u8; 3]) -> Vec<u8> {
    (0..h)
        .flat_map(|_| (0..w).flat_map(move |x| to.map(|c| (u32::from(c) * x as u32 / (w as u32 - 1)) as u8)))
        .collect()
}

fn render(stipple: &Stipple, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    stipple.render(buf.area, &mut buf);
    buf
}

fn dots_in(cell: &ratatui::buffer::Cell) -> u32 {
    cell.symbol()
        .chars()
        .next()
        .map(u32::from)
        .filter(|c| (0x2800..=0x28FF).contains(c))
        .map_or(0, |c| (c - 0x2800).count_ones())
}

#[test]
fn black_stays_black() {
    let px = solid(64, 32, [0, 0, 0]);
    let pic = Picture::new(64, 32, &px).unwrap();
    for seed in 0..5 {
        let buf = render(&Stipple::new(pic).seed(seed), 32, 16);
        for cell in &buf.content {
            assert_eq!(cell.symbol(), " ", "{buf:?}");
            assert_eq!(cell.bg, Color::Rgb(0, 0, 0));
        }
    }
}

#[test]
fn white_clips_to_solid_blocks() {
    let px = solid(64, 32, [255, 255, 255]);
    let pic = Picture::new(64, 32, &px).unwrap();
    let buf = render(&Stipple::new(pic).fit(Fit::Stretch), 32, 16);
    let [r, g, b] = Look::DEFAULT.palette[7];
    assert!(
        buf.content
            .iter()
            .all(|c| c.symbol() == " " && c.bg == Color::Rgb(r, g, b))
    );
}

#[test]
fn same_seed_same_frame_new_seed_new_marks() {
    let px = ramp(160, 80, [80, 220, 60]);
    let pic = Picture::new(160, 80, &px).unwrap();
    let a = render(&Stipple::new(pic).seed(1), 80, 20);
    assert_eq!(a, render(&Stipple::new(pic).seed(1), 80, 20));
    let b = render(&Stipple::new(pic).seed(2), 80, 20);
    assert_ne!(a, b);
    // Only the marks shimmer: the background stays put.
    let bgs = |buf: &Buffer| buf.content.iter().map(|c| c.bg).collect::<Vec<_>>();
    let changed = bgs(&a).iter().zip(bgs(&b)).filter(|(x, y)| *x != y).count();
    assert!(changed * 20 < a.content.len(), "{changed} backgrounds changed");
}

#[test]
fn dots_gather_in_the_bright_part() {
    let px = ramp(200, 50, [255, 255, 255]);
    let pic = Picture::new(200, 50, &px).unwrap();
    let look = Look {
        clip: 2.0,
        ..Look::DEFAULT
    };
    let buf = render(&Stipple::new(pic).look(&look).fit(Fit::Stretch), 100, 10);
    let column_dots = |x0: u16, x1: u16| -> u32 {
        (x0..x1)
            .flat_map(|x| (0..10).map(move |y| (x, y)))
            .map(|p| dots_in(&buf[p]))
            .sum()
    };
    let (dark, mid, bright) = (column_dots(0, 30), column_dots(40, 70), column_dots(80, 100));
    assert_eq!(dark, 0, "dots in the dark third");
    assert!(bright > mid && bright > 0, "{dark} {mid} {bright}");
}

#[test]
fn dot_colours_follow_the_channels() {
    for (rgb, code) in [([255, 0, 0], 1), ([0, 255, 0], 2), ([0, 0, 255], 4)] {
        let px = solid(40, 40, rgb);
        let pic = Picture::new(40, 40, &px).unwrap();
        let buf = render(&Stipple::new(pic), 20, 10);
        let [r, g, b] = Look::DEFAULT.palette[code];
        let dotted: Vec<_> = buf.content.iter().filter(|c| dots_in(c) > 0).collect();
        assert!(!dotted.is_empty(), "no dots for {rgb:?}");
        assert!(dotted.iter().all(|c| c.fg == Color::Rgb(r, g, b)), "{rgb:?}");
    }
}

#[test]
fn letterbox_is_plain_background() {
    // A wide picture in a tall area leaves bars above and below.
    let px = ramp(200, 50, [200, 120, 255]);
    let pic = Picture::new(200, 50, &px).unwrap();
    let look = Look {
        background: [1, 2, 3],
        ..Look::DEFAULT
    };
    let buf = render(&Stipple::new(pic).look(&look).seed(9), 40, 40);
    for y in [0, 1, 38, 39] {
        for x in 0..40 {
            let cell = &buf[(x, y)];
            assert_eq!((cell.symbol(), cell.bg), (" ", Color::Rgb(1, 2, 3)), "({x}, {y})");
        }
    }
}

#[test]
fn detail_splits_cells_along_edges() {
    // Dark stripes two dots tall: every cell straddles an edge, and nothing
    // is bright enough for dots.
    let mut px = Vec::new();
    for y in 0..64 {
        let v = if (y / 4) % 2 == 0 { 70 } else { 15 };
        px.extend(solid(64, 1, [v, v, v]));
    }
    let pic = Picture::new(64, 64, &px).unwrap();
    let look = Look {
        dash_density: 0.0,
        drip_density: 0.0,
        ..Look::DEFAULT
    };
    let levels = Levels {
        black: 0.0,
        white: 1.0,
        dots: 0.9,
    };
    let stipple = Stipple::new(pic).look(&look).levels(levels).fit(Fit::Stretch);
    let flat = render(&stipple.clone().detail(Detail::Flat), 16, 8);
    assert!(flat.content.iter().all(|c| c.symbol() == " "));
    let halves = render(&stipple.clone().detail(Detail::Halves), 16, 8);
    assert!(halves.content.iter().all(|c| c.symbol() == "▀"), "{halves:?}");
    let octants = render(&stipple.detail(Detail::Octants), 16, 8);
    assert!(octants.content.iter().all(|c| c.symbol() == "▀" && c.fg != c.bg));
}

#[test]
fn never_panics() {
    let px = ramp(37, 23, [255, 128, 7]);
    let pictures = [
        Picture::new(37, 23, &px).unwrap(),
        Picture::new(1, 1, &px[..3]).unwrap(),
        Picture::new(0, 0, &[]).unwrap(),
    ];
    let looks = [
        Look::DEFAULT,
        Look {
            dot_edge: 0.0,
            dot_ramp: 0.0,
            dot_threshold: (1.0, 0.0),
            base_levels: 0.0,
            base_gamma: 0.0,
            hue_contrast: 0.0,
            dash_glyphs: &[],
            drip_decay: 2.0,
            ..Look::DEFAULT
        },
    ];
    for pic in pictures {
        for look in &looks {
            for detail in [Detail::Flat, Detail::Halves, Detail::Quadrants, Detail::Octants] {
                for fit in [Fit::Contain, Fit::Cover, Fit::Stretch] {
                    for (w, h) in [(0, 0), (1, 1), (3, 1), (1, 7), (13, 5)] {
                        let s = Stipple::new(pic).look(look).detail(detail).fit(fit).cell_aspect(0.3);
                        render(&s, w, h);
                    }
                }
            }
        }
        // An area hanging off the buffer.
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 4));
        Stipple::new(pic).render(Rect::new(3, 2, 20, 20), &mut buf);
    }
}
