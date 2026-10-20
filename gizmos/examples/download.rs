//! Inline speed dials for simulated parallel downloads — a drop-in for the
//! rate in tqdm-style lines.
//!
//! Each stream gets two three-line `MiniSpeedometer`s side by side: one with
//! a precise readout, one with labelled gradations (`no_readout`).
//! Renders below your prompt (no alternate screen) and leaves the final state
//! on screen when every stream finishes. Progress, file names and the like
//! are deliberately left to other widgets.
//!
//! cargo run --example download      (q to quit early)

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::{Frame, TerminalOptions, Viewport};
use tui_gizmos::{Damped, MiniSpeedometer, PeakHold};

const STREAMS: usize = 3;
const ROW_HEIGHT: u16 = 3;
const SIZE: f64 = 2e9;

struct Stream {
    done: f64,
    rate: Damped,
    peak: PeakHold,
    seed: f64,
}

impl Stream {
    fn new(seed: f64) -> Self {
        Self {
            done: 0.0,
            rate: Damped::new(0.0, Duration::from_millis(300)),
            peak: PeakHold::new(Duration::from_secs(2), 0.7),
            seed,
        }
    }

    fn advance(&mut self, t: f64, dt: Duration) {
        let rate = if self.done >= SIZE {
            0.0
        } else {
            let wobble = ((t * 1.1 + self.seed).sin() + (t * 3.7 + self.seed).sin() * 0.4) / 1.4;
            1.2e8 * 10f64.powf(wobble * 0.8)
        };
        let rate = self.rate.update(rate, dt);
        self.peak.update(rate, dt);
        self.done = (self.done + rate * dt.as_secs_f64()).min(SIZE);
    }
}

fn draw(frame: &mut Frame, streams: &[Stream]) {
    let rows = Layout::vertical(streams.iter().map(|_| Constraint::Length(ROW_HEIGHT + 1))).split(frame.area());
    for (stream, area) in streams.iter().zip(rows.iter()) {
        frame.render_widget(
            MiniSpeedometer::bytes_per_sec(stream.rate.get()).peak(stream.peak.get()),
            Rect {
                height: ROW_HEIGHT,
                ..*area
            },
        );
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init_with_options(TerminalOptions {
        viewport: Viewport::Inline((ROW_HEIGHT + 1) * STREAMS as u16),
    });
    let mut streams: Vec<Stream> = (0..STREAMS).map(|i| Stream::new(i as f64 * 2.17)).collect();
    let start = Instant::now();
    let mut last = Instant::now();

    let result = (|| -> color_eyre::Result<()> {
        // Run until every stream has finished and its needle has settled.
        while streams.iter().any(|s| s.done < SIZE || s.rate.get() > 1e3) {
            terminal.draw(|f| draw(f, &streams))?;
            if event::poll(Duration::from_millis(33))?
                && let Event::Key(key) = event::read()?
                && (key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)))
            {
                return Ok(());
            }
            let dt = last.elapsed();
            last = Instant::now();
            let t = start.elapsed().as_secs_f64();
            for stream in &mut streams {
                stream.advance(t, dt);
            }
        }
        terminal.draw(|f| draw(f, &streams))?;
        Ok(())
    })();
    ratatui::restore();
    println!();
    result
}
