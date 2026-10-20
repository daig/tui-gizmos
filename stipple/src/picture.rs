//! Input frames, and fitting them onto the braille dot grid.

/// An RGB image: `width × height` pixels, three bytes each, row by row.
/// This is the layout `ffmpeg -f rawvideo -pix_fmt rgb24` writes.
#[derive(Debug, Clone, Copy)]
pub struct Picture<'a> {
    width: usize,
    height: usize,
    rgb: &'a [u8],
}

impl<'a> Picture<'a> {
    /// Wraps `rgb`, or returns `None` unless it is exactly
    /// `width * height * 3` bytes.
    #[must_use]
    pub fn new(width: usize, height: usize, rgb: &'a [u8]) -> Option<Self> {
        let len = width.checked_mul(height)?.checked_mul(3)?;
        (len == rgb.len()).then_some(Self { width, height, rgb })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// The pixel bytes.
    #[must_use]
    pub const fn rgb(&self) -> &'a [u8] {
        self.rgb
    }
}

/// How a picture fills an area of a different shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fit {
    /// Show the whole picture, centred, with background on either side.
    #[default]
    Contain,
    /// Fill the area, cropping whatever overflows.
    Cover,
    /// Fill the area, distorting the picture to fit.
    Stretch,
}

/// A picture resampled to one colour per braille dot.
pub(crate) struct Field {
    pub width: usize,
    /// The dots the picture covers, as `[x0, y0, x1, y1)`. Dots outside it
    /// are background.
    pub inside: [usize; 4],
    /// Colours in `0.0..=1.0`, row by row; black outside `inside`.
    pub px: Vec<[f32; 3]>,
}

/// Mean of `values` (a `width`-wide grid) over the square of `radius`
/// around each point, clipped at the edges.
pub(crate) fn box_mean(values: &[f32], width: usize, radius: usize) -> Vec<f32> {
    if width == 0 {
        return Vec::new();
    }
    let height = values.len() / width;
    // Summed-area table with a zero row and column in front.
    let w1 = width + 1;
    let mut sat = vec![0f64; w1 * (height + 1)];
    for y in 0..height {
        let mut row = 0f64;
        for x in 0..width {
            row += f64::from(values[y * width + x]);
            sat[(y + 1) * w1 + x + 1] = sat[y * w1 + x + 1] + row;
        }
    }
    let mut out = vec![0f32; values.len()];
    for y in 0..height {
        let (y0, y1) = (y.saturating_sub(radius), (y + radius + 1).min(height));
        for x in 0..width {
            let (x0, x1) = (x.saturating_sub(radius), (x + radius + 1).min(width));
            let sum = sat[y1 * w1 + x1] - sat[y0 * w1 + x1] - sat[y1 * w1 + x0] + sat[y0 * w1 + x0];
            out[y * width + x] = (sum / ((y1 - y0) * (x1 - x0)) as f64) as f32;
        }
    }
    out
}

impl Field {
    pub fn get(&self, x: usize, y: usize) -> [f32; 3] {
        self.px[y * self.width + x]
    }

    pub fn is_inside(&self, x: usize, y: usize) -> bool {
        let [x0, y0, x1, y1] = self.inside;
        (x0..x1).contains(&x) && (y0..y1).contains(&y)
    }
}

/// For each of `n` output samples, the input range `[start, end)` it
/// averages, covering `len` inputs from `offset` (fractional) onwards.
fn spans(n: usize, offset: f64, len: f64, limit: usize) -> Vec<(usize, usize)> {
    (0..n)
        .map(|i| {
            let a = offset + len * i as f64 / n as f64;
            let b = offset + len * (i + 1) as f64 / n as f64;
            let start = (a.floor().max(0.0) as usize).min(limit.saturating_sub(1));
            let end = (b.ceil() as usize).clamp(start + 1, limit);
            (start, end)
        })
        .collect()
}

/// Resamples `picture` onto a `width × height` dot grid by box filtering.
/// `dot_aspect` is a dot's width over its height on screen.
pub(crate) fn sample(picture: &Picture, width: usize, height: usize, fit: Fit, dot_aspect: f64) -> Field {
    let mut field = Field {
        width,
        inside: [0; 4],
        px: vec![[0.0; 3]; width * height],
    };
    if width == 0 || height == 0 || picture.width == 0 || picture.height == 0 {
        return field;
    }
    let (pw, ph) = (picture.width as f64, picture.height as f64);
    // The grid's shape in screen units, where a dot is `dot_aspect` wide and 1 tall.
    let (gw, gh) = (width as f64 * dot_aspect, height as f64);
    // Target rectangle in dots, and the source window in pixels.
    let (tw, th, sx, sy, sw, sh) = match fit {
        Fit::Stretch => (width, height, 0.0, 0.0, pw, ph),
        Fit::Contain => {
            let scale = (gw / pw).min(gh / ph);
            let tw = ((pw * scale / dot_aspect).round() as usize).clamp(1, width);
            let th = ((ph * scale).round() as usize).clamp(1, height);
            (tw, th, 0.0, 0.0, pw, ph)
        }
        Fit::Cover => {
            let scale = (gw / pw).max(gh / ph);
            let (sw, sh) = ((gw / scale).min(pw), (gh / scale).min(ph));
            (width, height, (pw - sw) / 2.0, (ph - sh) / 2.0, sw, sh)
        }
    };
    let (x0, y0) = ((width - tw) / 2, (height - th) / 2);
    field.inside = [x0, y0, x0 + tw, y0 + th];

    let cols = spans(tw, sx, sw, picture.width);
    let rows = spans(th, sy, sh, picture.height);
    let rgb = picture.rgb;
    let mut sums = vec![[0f32; 3]; tw];
    for (ty, &(r0, r1)) in rows.iter().enumerate() {
        sums.iter_mut().for_each(|s| *s = [0.0; 3]);
        for r in r0..r1 {
            let line = &rgb[r * picture.width * 3..(r + 1) * picture.width * 3];
            for (sum, &(c0, c1)) in sums.iter_mut().zip(&cols) {
                for px in line[c0 * 3..c1 * 3].as_chunks::<3>().0 {
                    sum[0] += f32::from(px[0]);
                    sum[1] += f32::from(px[1]);
                    sum[2] += f32::from(px[2]);
                }
            }
        }
        let out = &mut field.px[(y0 + ty) * width + x0..][..tw];
        for ((o, sum), &(c0, c1)) in out.iter_mut().zip(&sums).zip(&cols) {
            let n = ((r1 - r0) * (c1 - c0)) as f32 * 255.0;
            *o = sum.map(|s| s / n);
        }
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        rgb.repeat(w * h)
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(Picture::new(2, 2, &[0; 11]).is_none());
        assert!(Picture::new(2, 2, &[0; 12]).is_some());
        assert!(Picture::new(usize::MAX, 2, &[]).is_none());
    }

    #[test]
    fn contain_letterboxes_and_averages() {
        // A 4×2 picture, left half white, right half black, on a square grid.
        let mut px = solid(4, 2, [0, 0, 0]);
        for y in 0..2 {
            for x in 0..2 {
                px[(y * 4 + x) * 3..][..3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let pic = Picture::new(4, 2, &px).unwrap();
        let f = sample(&pic, 8, 8, Fit::Contain, 1.0);
        assert_eq!(f.inside, [0, 2, 8, 6]);
        assert_eq!(f.get(0, 3), [1.0; 3]);
        assert_eq!(f.get(7, 3), [0.0; 3]);
        assert_eq!(f.get(0, 0), [0.0; 3]);
        assert!(!f.is_inside(0, 1) && f.is_inside(0, 2));
    }

    #[test]
    fn cover_and_stretch_fill_the_grid() {
        let px = solid(10, 3, [255, 0, 0]);
        let pic = Picture::new(10, 3, &px).unwrap();
        for fit in [Fit::Cover, Fit::Stretch] {
            let f = sample(&pic, 6, 6, fit, 1.0);
            assert_eq!(f.inside, [0, 0, 6, 6]);
            assert!(f.px.iter().all(|p| *p == [1.0, 0.0, 0.0]));
        }
    }

    #[test]
    fn box_mean_averages_neighbourhoods() {
        let v = [0.0, 0.0, 3.0, 0.0, 0.0, 0.0];
        assert_eq!(box_mean(&v, 3, 0), v);
        let m = box_mean(&v, 3, 1);
        assert_eq!(m[0], 0.0);
        assert_eq!(m[1], 0.5);
        assert_eq!(m[5], 0.75);
        assert!(box_mean(&[], 0, 2).is_empty());
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        let px = solid(3, 1, [9, 9, 9]);
        let pic = Picture::new(3, 1, &px).unwrap();
        for (w, h) in [(0, 0), (1, 1), (1, 500), (500, 1), (7, 3)] {
            for fit in [Fit::Contain, Fit::Cover, Fit::Stretch] {
                let f = sample(&pic, w, h, fit, 1.0);
                assert_eq!(f.px.len(), w * h);
            }
        }
        let empty = Picture::new(0, 0, &[]).unwrap();
        assert_eq!(sample(&empty, 4, 4, Fit::Contain, 1.0).inside, [0; 4]);
    }
}
