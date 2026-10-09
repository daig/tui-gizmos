//! Typical download behaviour on the graduated `MiniSpeedometer`, next to
//! the two-row `ArcSpeedometer`.
//!
//! cargo run --example simulate
//!
//! A small model of how a multi-file download's rate really moves:
//!
//! - **connecting** — DNS, TLS, waiting on a lock: nothing flows.
//! - **slow start** — TCP doubles its rate every round trip until it finds
//!   the link's capacity.
//! - **steady** — congestion avoidance: the rate creeps up, a lost packet
//!   cuts it by about half, and it creeps up again (the classic sawtooth),
//!   with the link's own jitter on top.
//! - **stalled** — the server hiccups and the rate collapses, then TCP
//!   starts over from slow start.
//! - **cdn burst** — a cache node answers and the rate jumps for a moment.
//!
//! Between files the connection is torn down and set up again. Every so
//! often the simulated link changes, from fibre down to hotel wi-fi.
//!
//! Keys: `space` next link · `r` reveal the exact rate · `q` quit.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::{DefaultTerminal, Frame};
use tui_gizmos::{ArcSpeedometer, Damped, MiniSpeedometer, PeakHold, format};

const FRAME: Duration = Duration::from_millis(33);
const LINK_DURATION: f64 = 30.0;

/// A network link: capacity in bytes/second and round-trip time.
struct Link {
    name: &'static str,
    capacity: f64,
    rtt: f64,
}

const LINKS: [Link; 5] = [
    Link {
        name: "fibre",
        capacity: 110e6,
        rtt: 0.015,
    },
    Link {
        name: "home wi-fi",
        capacity: 22e6,
        rtt: 0.035,
    },
    Link {
        name: "mobile 4G",
        capacity: 6e6,
        rtt: 0.06,
    },
    Link {
        name: "hotel wi-fi",
        capacity: 900e3,
        rtt: 0.15,
    },
    Link {
        name: "datacenter",
        capacity: 600e6,
        rtt: 0.002,
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Connecting { left: f64 },
    SlowStart,
    Steady,
    Stalled { left: f64 },
}

/// xorshift64*: deterministic noise without pulling in a crate.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    /// True with the given rate of occurrence per second.
    fn chance(&mut self, per_sec: f64, dt: f64) -> bool {
        self.next() < per_sec * dt
    }
}

struct Sim {
    rng: Rng,
    link: usize,
    link_time: f64,
    phase: Phase,
    /// The rate TCP is currently allowing, before link jitter.
    window: f64,
    /// Seconds left of a CDN burst, during which capacity is higher.
    burst: f64,
    /// Seconds left showing "packet loss" after a cut.
    loss_flash: f64,
    file: u32,
    file_left: f64,
    /// What a progress bar would report: bytes over a short window.
    rate: f64,
}

impl Sim {
    fn new() -> Self {
        let mut sim = Self {
            rng: Rng(0x9E37_79B9_7F4A_7C15),
            link: 0,
            link_time: 0.0,
            phase: Phase::Steady,
            window: 0.0,
            burst: 0.0,
            loss_flash: 0.0,
            file: 0,
            file_left: 0.0,
            rate: 0.0,
        };
        sim.next_file();
        sim
    }

    fn link(&self) -> &'static Link {
        &LINKS[self.link]
    }

    fn next_link(&mut self) {
        self.link = (self.link + 1) % LINKS.len();
        self.link_time = 0.0;
        self.next_file();
    }

    fn next_file(&mut self) {
        self.file += 1;
        // Size files to take a handful of seconds on this link, so every
        // phase shows up regularly.
        self.file_left = self.link().capacity * self.rng.range(4.0, 12.0);
        self.phase = Phase::Connecting {
            left: self.rng.range(0.3, 1.2) + self.link().rtt * 4.0,
        };
        self.window = 0.0;
    }

    fn label(&self) -> &'static str {
        match self.phase {
            Phase::Connecting { .. } if self.file > 1 => "next file · connecting",
            Phase::Connecting { .. } => "connecting",
            Phase::SlowStart => "slow start",
            Phase::Stalled { .. } => "stalled",
            Phase::Steady if self.burst > 0.0 => "cdn burst",
            Phase::Steady if self.loss_flash > 0.0 => "packet loss",
            Phase::Steady => "steady",
        }
    }

    fn step(&mut self, dt: f64) {
        self.link_time += dt;
        if self.link_time > LINK_DURATION {
            self.next_link();
        }
        let Link { capacity, rtt, .. } = *self.link();
        self.burst = (self.burst - dt).max(0.0);
        self.loss_flash = (self.loss_flash - dt).max(0.0);
        let capacity = if self.burst > 0.0 { capacity * 2.5 } else { capacity };

        self.phase = match self.phase {
            Phase::Connecting { left } if left > dt => Phase::Connecting { left: left - dt },
            Phase::Connecting { .. } => {
                // Initial window: ten packets per round trip.
                self.window = 10.0 * 1460.0 / rtt;
                Phase::SlowStart
            }
            Phase::SlowStart => {
                // Doubles per round trip in theory; receive-window autotuning
                // makes real ramps take a second or so even on fast links.
                self.window *= 2f64.powf(dt / rtt.max(0.12));
                if self.window >= capacity {
                    self.window = capacity;
                    Phase::Steady
                } else {
                    Phase::SlowStart
                }
            }
            Phase::Steady => {
                // Additive increase toward capacity; multiplicative decrease
                // on loss, more likely the closer we run to the limit.
                self.window = (self.window + capacity * 0.08 * dt).min(capacity);
                if self.rng.chance(0.6 * self.window / capacity, dt) {
                    self.window *= self.rng.range(0.45, 0.7);
                    self.loss_flash = 0.6;
                }
                if self.rng.chance(0.04, dt) {
                    Phase::Stalled {
                        left: self.rng.range(1.0, 3.0),
                    }
                } else {
                    if self.burst == 0.0 && self.rng.chance(0.03, dt) {
                        self.burst = self.rng.range(1.5, 3.0);
                    }
                    Phase::Steady
                }
            }
            Phase::Stalled { left } if left > dt => {
                self.window *= 0.02f64.powf(dt);
                Phase::Stalled { left: left - dt }
            }
            Phase::Stalled { .. } => {
                self.window = self.window.max(10.0 * 1460.0 / rtt);
                Phase::SlowStart
            }
        };

        let flowing = !matches!(self.phase, Phase::Connecting { .. });
        let jitter = if flowing { self.rng.range(0.85, 1.12) } else { 0.0 };
        let instant = (self.window * jitter).min(capacity * 1.1);
        self.file_left -= instant * dt;
        if self.file_left <= 0.0 {
            self.next_file();
        }
        // Progress bars average over a short window; so does this.
        let alpha = 1.0 - (-dt / 0.3).exp();
        self.rate += (instant - self.rate) * alpha;
    }
}

struct App {
    sim: Sim,
    needle: Damped,
    peak: PeakHold,
    reveal: bool,
}

impl App {
    fn draw(&self, frame: &mut Frame) {
        let link = self.sim.link();
        let dial = MiniSpeedometer::bytes_per_sec(self.needle.get())
            .peak(self.peak.get())
            .no_readout();
        let arc = ArcSpeedometer::bytes_per_sec(self.needle.get()).peak(self.peak.get());

        let [_, title, _, gauge, _, phase, exact, _, help] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let [dial_area, arc_area] =
            Layout::horizontal([Constraint::Length(dial.dial_width(3)), Constraint::Length(arc.width(2))])
                .spacing(8)
                .flex(Flex::Center)
                .areas(gauge);
        // Bottom-align the two-row arc with the three-row dial.
        let arc_area = Rect {
            y: arc_area.y + 1,
            height: 2,
            ..arc_area
        };

        frame.render_widget(
            Line::from(vec![
                Span::raw(link.name).bold(),
                Span::raw(format!("  ·  {} link", format::bytes_per_sec(link.capacity))).dark_gray(),
            ])
            .centered(),
            title,
        );
        frame.render_widget(dial, dial_area);
        frame.render_widget(arc, arc_area);
        frame.render_widget(Line::from(self.sim.label()).dark_gray().centered(), phase);
        if self.reveal {
            frame.render_widget(
                Line::from(format::bytes_per_sec(self.sim.rate))
                    .dark_gray()
                    .italic()
                    .centered(),
                exact,
            );
        }
        frame.render_widget(
            Line::from("space next link · r reveal exact rate · q quit")
                .dark_gray()
                .centered(),
            help,
        );
    }
}

fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut app = App {
        sim: Sim::new(),
        needle: Damped::new(0.0, Duration::from_millis(200)),
        peak: PeakHold::new(Duration::from_secs(2), 0.6),
        reveal: false,
    };
    let mut last = Instant::now();
    loop {
        terminal.draw(|f| app.draw(f))?;
        if event::poll(FRAME)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char(' ') => app.sim.next_link(),
                KeyCode::Char('r') => app.reveal = !app.reveal,
                _ => {}
            }
        }
        let dt = last.elapsed();
        last = Instant::now();
        // Step the model finely; it has round trips as short as 2 ms.
        let steps = (dt.as_secs_f64() / 0.005).ceil().max(1.0);
        for _ in 0..steps as u32 {
            app.sim.step(dt.as_secs_f64() / steps);
        }
        let rate = app.needle.update(app.sim.rate, dt);
        app.peak.update(rate, dt);
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs every link for its full duration and checks each phase shows up
    /// and the reported rate stays within the link's (burst) capacity.
    #[test]
    fn model_covers_every_phase() {
        let mut sim = Sim::new();
        let mut seen = std::collections::HashSet::new();
        let dt = 0.005;
        let mut t = 0.0;
        while t < LINK_DURATION * LINKS.len() as f64 {
            let link = sim.link;
            sim.step(dt);
            t += dt;
            seen.insert(sim.label());
            // Right after a switch the averaged rate still carries the old link.
            if sim.link == link && sim.link_time > 1.0 {
                assert!(sim.rate <= LINKS[link].capacity * 2.8, "{} at {t:.1}s", sim.rate);
            }
            if std::env::var("SIM_TRACE").is_ok() && (t * 4.0).fract() < dt * 4.0 {
                println!(
                    "{t:6.2}s {:>12} {:>10} {}",
                    LINKS[sim.link].name,
                    format::bytes_per_sec(sim.rate),
                    sim.label()
                );
            }
        }
        for label in [
            "connecting",
            "next file · connecting",
            "slow start",
            "steady",
            "packet loss",
            "stalled",
            "cdn burst",
        ] {
            assert!(seen.contains(label), "never saw {label}: {seen:?}");
        }
    }
}
