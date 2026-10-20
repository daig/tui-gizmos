//! Plays a GIF or video in the terminal as stipple art.
//!
//! stipple [options] <file>

mod video;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use ratatui::{DefaultTerminal, Frame};
use tui_stipple::{AutoLevels, Detail, Fit, Glitch, Look, Picture, Stipple};
use video::{Info, fit_within, frames, probe};

const USAGE: &str = "\
Plays a GIF or video in the terminal as stipple art. Needs ffmpeg.

usage: stipple [options] <file>

options:
  --fit contain|cover|stretch   how the picture fills the terminal (contain)
  --detail flat|halves|quadrants|octants
                                block glyphs for detail within a cell; octants need a
                                terminal that draws them (default: octants in Ghostty,
                                quadrants elsewhere)
  --accents                     add magenta dashes in the shadows and red drips
  --glitch <rate>               add glitches, this many times the usual rate (off)
  --shimmer <hz>                re-deal the dots this often even when the picture
                                is still (off: they change with each new frame)
  --fps <n>                     playback rate (the file's own, at most 30)
  --start <seconds>             start this far in
  --seed <n>                    first seed (0)
  --dump <path>                 write rendered frames as text instead of playing
  --size <cols>x<rows>          terminal size for --dump (100x50)
  --frames <n>                  stop --dump after n frames

keys: space pause · s shimmer · a accents · g glitch · d detail · f fit · ? help · q quit";

/// Shimmer rate the `s` key turns on, unless `--shimmer` gave one.
const SHIMMER_HZ: f64 = 15.0;
/// How long a key's label stays on screen.
const TOAST: Duration = Duration::from_millis(1500);

struct Args {
    path: PathBuf,
    fit: Fit,
    detail: Detail,
    accents: bool,
    glitch: f32,
    shimmer: f64,
    fps: Option<f64>,
    start: f64,
    seed: u64,
    dump: Option<PathBuf>,
    size: (u16, u16),
    frames: Option<usize>,
}

/// Octants where the terminal is known to draw them (Ghostty), quadrants
/// elsewhere.
fn default_detail() -> Detail {
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default().to_lowercase();
    let term = std::env::var("TERM").unwrap_or_default();
    if program == "ghostty" || term == "xterm-ghostty" {
        Detail::Octants
    } else {
        Detail::Quadrants
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut parsed = Args {
        path: PathBuf::new(),
        fit: Fit::Contain,
        detail: default_detail(),
        accents: false,
        glitch: 0.0,
        shimmer: 0.0,
        fps: None,
        start: 0.0,
        seed: 0,
        dump: None,
        size: (100, 50),
        frames: None,
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        let number = |name: &str, v: String| v.parse::<f64>().map_err(|_| format!("{name}: not a number: {v}"));
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--fit" => {
                parsed.fit = match value("--fit")?.as_str() {
                    "contain" => Fit::Contain,
                    "cover" => Fit::Cover,
                    "stretch" => Fit::Stretch,
                    other => return Err(format!("--fit: unknown mode {other}")),
                }
            }
            "--detail" => {
                parsed.detail = match value("--detail")?.as_str() {
                    "flat" => Detail::Flat,
                    "halves" => Detail::Halves,
                    "quadrants" => Detail::Quadrants,
                    "octants" => Detail::Octants,
                    other => return Err(format!("--detail: unknown level {other}")),
                }
            }
            "--accents" => parsed.accents = true,
            "--glitch" => parsed.glitch = number("--glitch", value("--glitch")?)?.max(0.0) as f32,
            "--shimmer" => parsed.shimmer = number("--shimmer", value("--shimmer")?)?.clamp(0.0, 60.0),
            "--fps" => parsed.fps = Some(number("--fps", value("--fps")?)?.clamp(1.0, 60.0)),
            "--start" => parsed.start = number("--start", value("--start")?)?.max(0.0),
            "--seed" => parsed.seed = number("--seed", value("--seed")?)? as u64,
            "--dump" => parsed.dump = Some(value("--dump")?.into()),
            "--size" => {
                let v = value("--size")?;
                let size = v
                    .split_once('x')
                    .and_then(|(c, r)| Some((c.parse().ok()?, r.parse().ok()?)))
                    .filter(|&(c, r)| c > 0 && r > 0);
                parsed.size = size.ok_or(format!("--size: expected <cols>x<rows>, got {v}"))?;
            }
            "--frames" => parsed.frames = Some(number("--frames", value("--frames")?)? as usize),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            file => path = Some(PathBuf::from(file)),
        }
    }
    parsed.path = path.ok_or("no file given")?;
    Ok(parsed)
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            if !err.is_empty() {
                eprintln!("stipple: {err}\n");
            }
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if !args.path.exists() {
        eprintln!("stipple: {}: no such file", args.path.display());
        std::process::exit(1);
    }
    let result = probe(&args.path).and_then(|info| match &args.dump {
        Some(out) => dump(&args, &info, out),
        None => {
            let mut terminal = ratatui::init();
            let result = play(&mut terminal, &args, &info);
            ratatui::restore();
            result
        }
    });
    if let Err(err) = result {
        eprintln!("stipple: {err}");
        std::process::exit(1);
    }
}

/// Decode size for a terminal area: twice the dot resolution, so every dot
/// averages a few pixels.
fn decode_size(info: &Info, cols: u16, rows: u16) -> (usize, usize) {
    fit_within(info, usize::from(cols) * 4, usize::from(rows) * 8)
}

fn playback_fps(args: &Args, info: &Info) -> f64 {
    args.fps.unwrap_or(info.fps.min(30.0))
}

const fn look_for(accents: bool) -> &'static Look {
    if accents { &Look::ACCENTED } else { &Look::DEFAULT }
}

/// Renders frames into a text file: a `frame <cols> <rows>` line, then one
/// `symbol fg bg` line per cell, row by row. Each frame gets a new seed.
fn dump(args: &Args, info: &Info, out: &Path) -> io::Result<()> {
    let (cols, rows) = args.size;
    let (w, h) = decode_size(info, cols, rows);
    let fps = playback_fps(args, info);
    let dt = Duration::from_secs_f64(1.0 / fps);
    // With a frame limit, loop so still images give that many frames too.
    let incoming = frames(&args.path, w, h, fps, args.start, args.frames.is_some());
    let mut file = BufWriter::new(File::create(out)?);
    let mut levels = AutoLevels::default();
    let mut glitch = Glitch::new(args.seed ^ 0x5EED).rate(args.glitch);
    let look = look_for(args.accents);
    let area = Rect::new(0, 0, cols, rows);
    let mut n = 0;
    while args.frames.is_none_or(|max| n < max)
        && let Ok(rgb) = incoming.recv()
    {
        let picture = Picture::new(w, h, &rgb).expect("decoder frame size");
        let lv = levels.update(&picture, look, dt);
        let mut buf = Buffer::empty(area);
        Stipple::new(picture)
            .look(look)
            .levels(lv)
            .fit(args.fit)
            .detail(args.detail)
            .seed(args.seed + n as u64)
            .render(area, &mut buf);
        glitch.apply(area, &mut buf, dt);
        writeln!(file, "frame\t{cols}\t{rows}")?;
        for cell in &buf.content {
            writeln!(file, "{}\t{}\t{}", cell.symbol(), hex(cell.fg), hex(cell.bg))?;
        }
        n += 1;
    }
    file.flush()?;
    eprintln!("{n} frames → {}", out.display());
    Ok(())
}

fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("{r:02x}{g:02x}{b:02x}"),
        _ => "000000".into(),
    }
}

struct Player {
    frame: Option<Arc<Vec<u8>>>,
    size: (usize, usize),
    levels: AutoLevels,
    accents: bool,
    fit: Fit,
    detail: Detail,
    /// Seed for the current dots: bumped by each new frame and each shimmer.
    seed: u64,
    shimmer: Option<f64>,
    shimmer_hz: f64,
    glitch: Glitch,
    glitching: bool,
    last_draw: Instant,
    paused: bool,
    help: bool,
    toast: Option<(String, Instant)>,
}

impl Player {
    /// Takes a decoded frame, returning whether it differs from the one
    /// showing. A still image arrives as the same frame over and over.
    fn show(&mut self, rgb: Arc<Vec<u8>>, dt: Duration) -> bool {
        if self.frame.as_ref().is_some_and(|f| Arc::ptr_eq(f, &rgb)) {
            return false;
        }
        if let Some(picture) = Picture::new(self.size.0, self.size.1, &rgb) {
            self.levels.update(&picture, look_for(self.accents), dt);
        }
        self.frame = Some(rgb);
        self.seed = self.seed.wrapping_add(1);
        true
    }

    fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now() + TOAST));
    }

    fn on_off(on: bool) -> &'static str {
        if on { "on" } else { "off" }
    }

    /// Handles a key, returning `false` to quit.
    fn key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return false,
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                self.say(if self.paused { "paused" } else { "playing" });
            }
            KeyCode::Char('s') => {
                self.shimmer = match self.shimmer {
                    Some(_) => None,
                    None => Some(self.shimmer_hz),
                };
                let message = match self.shimmer {
                    Some(hz) => format!("shimmer on, {hz} Hz"),
                    None => "shimmer off".into(),
                };
                self.say(message);
            }
            KeyCode::Char('a') => {
                self.accents = !self.accents;
                self.say(format!("accents {}", Self::on_off(self.accents)));
            }
            KeyCode::Char('g') => {
                self.glitching = !self.glitching;
                self.say(format!("glitches {}", Self::on_off(self.glitching)));
            }
            KeyCode::Char('d') => {
                self.detail = match self.detail {
                    Detail::Flat => Detail::Halves,
                    Detail::Halves => Detail::Quadrants,
                    Detail::Quadrants => Detail::Octants,
                    Detail::Octants => Detail::Flat,
                };
                self.say(format!("detail: {:?}", self.detail).to_lowercase());
            }
            KeyCode::Char('f') => {
                self.fit = match self.fit {
                    Fit::Contain => Fit::Cover,
                    Fit::Cover => Fit::Stretch,
                    Fit::Stretch => Fit::Contain,
                };
                self.say(format!("fit: {:?}", self.fit).to_lowercase());
            }
            KeyCode::Char('?' | 'h') => self.help = !self.help,
            _ => {}
        }
        true
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        if let Some(rgb) = &self.frame
            && let Some(picture) = Picture::new(self.size.0, self.size.1, rgb)
            && let Some(levels) = self.levels.get()
        {
            Stipple::new(picture)
                .look(look_for(self.accents))
                .levels(levels)
                .fit(self.fit)
                .detail(self.detail)
                .seed(self.seed)
                .render(area, buf);
        }
        let dt = self.last_draw.elapsed();
        self.last_draw = Instant::now();
        if self.glitching && !self.paused {
            self.glitch.apply(area, buf, dt);
        }
        if self.help {
            let lines = [
                "space  pause",
                "s      shimmer",
                "a      accents",
                "g      glitches",
                "d      detail",
                "f      fit",
                "?      help",
                "q      quit",
            ];
            let w = 20.min(area.width);
            let h = (lines.len() as u16 + 2).min(area.height);
            let rect = Rect::new(
                area.x + area.width.saturating_sub(w) / 2,
                area.y + area.height.saturating_sub(h) / 2,
                w,
                h,
            );
            Clear.render(rect, buf);
            Paragraph::new(lines.map(Line::from).to_vec())
                .block(Block::bordered().title(" stipple "))
                .white()
                .on_black()
                .render(rect, buf);
        }
        if let Some((message, _)) = &self.toast {
            let text = format!(" {message} ");
            let w = (text.chars().count() as u16).min(area.width);
            let rect = Rect::new(area.x, area.bottom().saturating_sub(1), w, 1.min(area.height));
            Clear.render(rect, buf);
            Line::from(text).white().on_black().render(rect, buf);
        }
    }
}

fn play(terminal: &mut DefaultTerminal, args: &Args, info: &Info) -> io::Result<()> {
    let area = terminal.size()?;
    let (w, h) = decode_size(info, area.width, area.height);
    let fps = playback_fps(args, info);
    let incoming = frames(&args.path, w, h, fps, args.start, true);

    let mut player = Player {
        frame: None,
        size: (w, h),
        levels: AutoLevels::default(),
        accents: args.accents,
        fit: args.fit,
        detail: args.detail,
        seed: args.seed,
        shimmer: (args.shimmer > 0.0).then_some(args.shimmer),
        shimmer_hz: if args.shimmer > 0.0 { args.shimmer } else { SHIMMER_HZ },
        glitch: Glitch::new(args.seed ^ 0x5EED).rate(if args.glitch > 0.0 { args.glitch } else { 1.0 }),
        glitching: args.glitch > 0.0,
        last_draw: Instant::now(),
        paused: false,
        help: false,
        toast: None,
    };
    let frame_time = Duration::from_secs_f64(1.0 / fps);
    let mut next_frame = Instant::now();
    let mut next_shimmer = Instant::now();
    let mut dirty = true;
    loop {
        let now = Instant::now();
        if !player.paused {
            if now >= next_frame {
                match incoming.try_recv() {
                    Ok(rgb) => dirty |= player.show(rgb, frame_time),
                    Err(mpsc::TryRecvError::Disconnected) if player.frame.is_none() => {
                        return Err(io::Error::other("ffmpeg produced no frames"));
                    }
                    Err(_) => {}
                }
                // Glitches move on every frame, even over a still picture.
                dirty |= player.glitching;
                next_frame = (next_frame + frame_time).max(now);
            }
            if let Some(hz) = player.shimmer
                && now >= next_shimmer
            {
                player.seed = player.seed.wrapping_add(1);
                next_shimmer = (next_shimmer + Duration::from_secs_f64(1.0 / hz)).max(now);
                dirty = true;
            }
        }
        if player.toast.as_ref().is_some_and(|(_, until)| now >= *until) {
            player.toast = None;
            dirty = true;
        }
        if dirty {
            terminal.draw(|f| player.draw(f))?;
            dirty = false;
        }

        // Sleep until the next thing to do, or a key.
        let mut wake = now + Duration::from_secs(1);
        if !player.paused {
            wake = wake.min(next_frame);
            if player.shimmer.is_some() {
                wake = wake.min(next_shimmer);
            }
        }
        if let Some((_, until)) = &player.toast {
            wake = wake.min(*until);
        }
        if event::poll(wake.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if !player.key(key.code) {
                        return Ok(());
                    }
                    if player.shimmer.is_some() {
                        next_shimmer = Instant::now();
                    }
                }
                Event::Resize(..) => {}
                _ => continue,
            }
            dirty = true;
        }
    }
}
