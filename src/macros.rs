/// Builder methods and helpers shared by every dial gizmo. Expects the fields
/// `value, min, max, scale, peak, fill, track_color, peak_color, readout,
/// readout_format, readout_style, unit_style, block, style`.
macro_rules! dial_builder {
    ($ty:ident) => {
        impl<'a> $ty<'a> {
            /// Sets the current value.
            #[must_use]
            pub const fn value(mut self, value: f64) -> Self {
                self.value = value;
                self
            }

            /// Sets the values at the start and end of the dial (default
            /// `0..100`). Values outside the range pin the needle to the
            /// nearest end.
            #[must_use]
            pub const fn range(mut self, min: f64, max: f64) -> Self {
                self.min = min;
                self.max = max;
                self
            }

            /// Sets linear or logarithmic mapping (default linear).
            #[must_use]
            pub const fn scale(mut self, scale: $crate::Scale) -> Self {
                self.scale = scale;
                self
            }

            /// Marks a peak value on the arc (e.g. the fastest rate seen).
            #[must_use]
            pub const fn peak(mut self, peak: f64) -> Self {
                self.peak = Some(peak);
                self
            }

            /// Sets how the filled arc is coloured (default: cyan→red gradient).
            #[must_use]
            pub fn fill(mut self, fill: $crate::Fill) -> Self {
                self.fill = fill;
                self
            }

            /// Shorthand for [`Fill::Solid`]($crate::Fill::Solid).
            #[must_use]
            pub fn fill_color(self, color: ::ratatui::style::Color) -> Self {
                self.fill($crate::Fill::Solid(color))
            }

            /// Colour of the unfilled arc (default dark gray).
            #[must_use]
            pub const fn track_color(mut self, color: ::ratatui::style::Color) -> Self {
                self.track_color = color;
                self
            }

            /// Colour of the peak marker (default light magenta).
            #[must_use]
            pub const fn peak_color(mut self, color: ::ratatui::style::Color) -> Self {
                self.peak_color = color;
                self
            }

            /// Sets the readout text, e.g. `"49.6 MB/s"`. Takes precedence
            /// over `readout_format`.
            #[must_use]
            pub fn readout(mut self, readout: impl Into<::ratatui::text::Line<'a>>) -> Self {
                self.readout = Some(readout.into());
                self
            }

            /// Formats the value into the readout when no explicit readout is set.
            #[must_use]
            pub fn readout_format(mut self, format: fn(f64) -> String) -> Self {
                self.readout_format = Some(format);
                self
            }

            /// Removes the readout, including one set by a preset.
            #[must_use]
            pub fn no_readout(mut self) -> Self {
                self.readout = None;
                self.readout_format = None;
                self
            }

            /// Base style of the readout (default bold).
            #[must_use]
            pub const fn readout_style(mut self, style: ::ratatui::style::Style) -> Self {
                self.readout_style = style;
                self
            }

            /// Style of the unit in a formatted readout — `MB/s` in
            /// `49.6 MB/s` — and of scale labels (default dark gray).
            #[must_use]
            pub const fn unit_style(mut self, style: ::ratatui::style::Style) -> Self {
                self.unit_style = style;
                self
            }

            /// Wraps the gizmo in a [`Block`](::ratatui::widgets::Block).
            #[must_use]
            pub fn block(mut self, block: ::ratatui::widgets::Block<'a>) -> Self {
                self.block = Some(block);
                self
            }

            /// Base style for the whole area.
            #[must_use]
            pub fn style(mut self, style: impl Into<::ratatui::style::Style>) -> Self {
                self.style = style.into();
                self
            }

            /// Where `value` falls on the dial, from 0.0 (start) to 1.0 (end).
            #[must_use]
            pub fn fraction_of(&self, value: f64) -> f64 {
                self.scale.fraction(value, self.min, self.max)
            }

            /// The styled readout. A formatted one is split at its first
            /// space so the number stands out from its unit.
            fn readout_line(&self) -> Option<::ratatui::text::Line<'a>> {
                use ::ratatui::text::{Line, Span};
                if let Some(line) = &self.readout {
                    return Some(line.clone().patch_style(self.readout_style));
                }
                let text = (self.readout_format?)(self.value);
                Some(match text.split_once(' ') {
                    Some((number, unit)) => Line::from(vec![
                        Span::styled(number.to_string(), self.readout_style),
                        Span::styled(format!(" {unit}"), self.unit_style),
                    ]),
                    None => Line::styled(text, self.readout_style),
                })
            }

            /// Applies the base style and block, returning the inner area.
            fn render_frame(
                &self,
                area: ::ratatui::layout::Rect,
                buf: &mut ::ratatui::buffer::Buffer,
            ) -> ::ratatui::layout::Rect {
                use ::ratatui::widgets::Widget as _;
                let area = area.intersection(buf.area);
                buf.set_style(area, self.style);
                match &self.block {
                    Some(block) => {
                        block.render(area, buf);
                        block.inner(area)
                    }
                    None => area,
                }
            }
        }
    };
}

/// Builder methods and helpers for dials with a needle and ticks. Expects the
/// fields `ticks, tick_color, needle_color, tick_labels` on top of those
/// [`dial_builder`] needs.
macro_rules! needle_builder {
    ($ty:ident) => {
        impl<'a> $ty<'a> {
            /// Sets the number of tick intervals; `ticks + 1` marks are drawn
            /// on dials big enough to show them. `0` hides them.
            #[must_use]
            pub const fn ticks(mut self, ticks: u16) -> Self {
                self.ticks = ticks;
                self
            }

            /// Colour of the tick marks (default gray).
            #[must_use]
            pub const fn tick_color(mut self, color: ::ratatui::style::Color) -> Self {
                self.tick_color = color;
                self
            }

            /// Colour of the needle and hub (default white).
            #[must_use]
            pub const fn needle_color(mut self, color: ::ratatui::style::Color) -> Self {
                self.needle_color = color;
                self
            }

            /// Labels every tick with its value, formatted by `format` (e.g.
            /// [`format::si`]($crate::format::si) gives `1k`, `10k`, … `1G`). Labels sit just inside the
            /// ticks, appear once the dial is big enough, and drop out where they
            /// would collide.
            #[must_use]
            pub fn tick_labels(mut self, format: fn(f64) -> String) -> Self {
                self.tick_labels = Some(format);
                self
            }

            /// What to draw, with `ticks` tick marks.
            fn paint(&self, ticks: u16) -> $crate::dial::Paint<'_> {
                $crate::dial::Paint {
                    value: self.fraction_of(self.value),
                    peak: self.peak.map(|p| self.fraction_of(p)),
                    ticks,
                    fill: &self.fill,
                    track_color: self.track_color,
                    tick_color: self.tick_color,
                    needle_color: self.needle_color,
                    peak_color: self.peak_color,
                }
            }

            /// Places a label inside each tick, `inset` dots in from the outer
            /// edge or further if it would touch the ring or tick marks.
            /// Labels that would crowd each other are dropped. Returns
            /// area-relative cells and text.
            fn place_tick_labels(
                &self,
                area: ::ratatui::layout::Rect,
                grid: &$crate::braille::BrailleGrid,
                g: &$crate::dial::Geometry,
                format: fn(f64) -> String,
                inset: f64,
            ) -> Vec<(::ratatui::layout::Rect, String)> {
                use ::ratatui::layout::Rect;
                let mut placed: Vec<(Rect, String)> = Vec::new();
                for i in 0..=self.ticks {
                    let fraction = f64::from(i) / f64::from(self.ticks);
                    let text = format(self.scale.value_at(fraction, self.min, self.max));
                    let width = text.chars().count() as f64;
                    let angle = g.angle_at(fraction);
                    let spot = (0..6).find_map(|step| {
                        let (x, y) = g.point(inset + f64::from(step), angle);
                        // Nudge sideways by half the label so labels near 3
                        // and 9 o'clock hang inward rather than over the ring.
                        let col = (x / 2.0 - width / 2.0 - angle.sin() * width / 2.0).round() as i32;
                        let row = (y / 4.0).floor() as i32;
                        if col < 0 || row < 0 {
                            return None;
                        }
                        let rect = Rect::new(col as u16, row as u16, width as u16, 1);
                        let fits = rect.right() <= area.width && rect.bottom() <= area.height;
                        let clear =
                            (rect.left()..rect.right()).all(|c| !grid.drawn_below(c, rect.y, $crate::dial::NEEDLE));
                        (fits && clear).then_some(rect)
                    });
                    let Some(rect) = spot else { continue };
                    let padded = Rect {
                        x: rect.x.saturating_sub(1),
                        width: rect.width + 2,
                        ..rect
                    };
                    if placed.iter().any(|(r, _)| r.intersects(padded)) {
                        continue;
                    }
                    placed.push((rect, text));
                }
                placed
            }

            /// Draws the needle around `labels`, renders the grid, then writes
            /// the labels.
            fn finish_dial(
                &self,
                buf: &mut ::ratatui::buffer::Buffer,
                area: ::ratatui::layout::Rect,
                mut grid: $crate::braille::BrailleGrid,
                g: &$crate::dial::Geometry,
                paint: &$crate::dial::Paint<'_>,
                labels: &[(::ratatui::layout::Rect, String)],
            ) {
                use ::ratatui::layout::Position;
                paint.draw_needle(&mut grid, g, |c, r| {
                    labels.iter().any(|(rect, _)| rect.contains(Position::new(c, r)))
                });
                grid.render(area, buf);
                for (rect, text) in labels {
                    buf.set_string(area.x + rect.x, area.y + rect.y, text, self.unit_style);
                }
            }
        }
    };
}

pub(crate) use {dial_builder, needle_builder};
