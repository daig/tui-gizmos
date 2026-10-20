//! Dashboard of speedometers driven by simulated signals.
//!
//! cargo run --example speedometer      (q / Esc to quit)

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};
use tui_gizmos::{Damped, Fill, PeakHold, Speedometer, format};

const FRAME: Duration = Duration::from_millis(33);

/// Smooth pseudo-random wobble in -1..1.
fn wobble(t: f64, seed: f64) -> f64 {
    ((t * 0.9 + seed).sin() + (t * 2.3 + seed * 1.7).sin() * 0.5 + (t * 6.1 + seed * 0.3).sin() * 0.25) / 1.75
}

struct App {
    start: Instant,
    rate: Damped,
    rate_peak: PeakHold,
    cpu: Damped,
    tokens: Damped,
}

impl App {
    fn new() -> Self {
        let tau = Duration::from_millis(250);
        Self {
            start: Instant::now(),
            rate: Damped::new(0.0, tau),
            rate_peak: PeakHold::new(Duration::from_secs(2), 0.6),
            cpu: Damped::new(0.0, tau),
            tokens: Damped::new(0.0, tau),
        }
    }

    fn tick(&mut self, dt: Duration) {
        let t = self.start.elapsed().as_secs_f64();
        // Download: 2–400 MB/s, with an occasional stall.
        let stall = if (t % 11.0) > 9.5 { 1e-3 } else { 1.0 };
        let rate = 3e7 * 10f64.powf(wobble(t, 0.0) * 1.2) * stall;
        self.rate_peak.update(self.rate.update(rate, dt), dt);
        self.cpu.update(55.0 + 40.0 * wobble(t, 4.0), dt);
        self.tokens.update(90.0 + 80.0 * wobble(t * 0.7, 9.0), dt);
    }

    fn draw(&self, frame: &mut Frame) {
        let [title, top, bottom, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(Line::from(" tui-gizmos · Speedometer ").bold().centered(), title);
        frame.render_widget(Line::from("q quit").dark_gray().centered(), help);

        let [a, b, c] = Layout::horizontal([Constraint::Fill(1); 3]).areas(top);
        frame.render_widget(
            Speedometer::bytes_per_sec(self.rate.get())
                .peak(self.rate_peak.get())
                .block(Block::bordered().title(" Network ").dark_gray()),
            a,
        );
        frame.render_widget(
            Speedometer::new(self.cpu.get())
                .sweep(180.0)
                .ticks(4)
                .readout(format!("{:.0}%", self.cpu.get()))
                .fill_color(Color::Rgb(167, 139, 250))
                .block(Block::bordered().title(" Half dial ").dark_gray()),
            b,
        );
        frame.render_widget(
            Speedometer::new(self.tokens.get())
                .range(0.0, 200.0)
                .sweep(300.0)
                .ticks(10)
                .thickness(2)
                .readout(format!("{:.0} tok/s", self.tokens.get()))
                .tick_labels(format::si)
                .fill(Fill::Gradient(vec![
                    (0.0, Color::Rgb(244, 114, 182)),
                    (1.0, Color::Rgb(251, 191, 36)),
                ]))
                .needle_color(Color::Rgb(251, 191, 36))
                .block(Block::bordered().title(" 300° sweep ").dark_gray()),
            c,
        );

        // The same dial at shrinking sizes.
        let sizes = [24, 20, 16, 12];
        let areas = Layout::horizontal(sizes.map(Constraint::Length))
            .spacing(2)
            .flex(ratatui::layout::Flex::Center)
            .split(bottom);
        for (area, w) in areas.iter().zip(sizes) {
            let [dial, caption] = Layout::vertical([Constraint::Length(w / 3 + 1), Constraint::Length(1)]).areas(*area);
            frame.render_widget(
                Speedometer::bytes_per_sec(self.rate.get()).peak(self.rate_peak.get()),
                dial,
            );
            frame.render_widget(Line::from(format!("{w}×{}", w / 3 + 1)).dark_gray().centered(), caption);
        }
    }
}

fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new();
    let mut last = Instant::now();
    loop {
        terminal.draw(|f| app.draw(f))?;
        if event::poll(FRAME)?
            && let Event::Key(key) = event::read()?
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
        app.tick(last.elapsed());
        last = Instant::now();
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    Ok(result?)
}
