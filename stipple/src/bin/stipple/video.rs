//! Decoding with `ffmpeg`, which reads nearly anything: GIF, MP4, WebM,
//! still images.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// What `ffprobe` reports about a file's video stream.
#[derive(Debug, Clone, Copy)]
pub struct Info {
    pub width: usize,
    pub height: usize,
    pub fps: f64,
}

fn missing(tool: &str, err: io::Error) -> io::Error {
    if err.kind() == io::ErrorKind::NotFound {
        io::Error::new(err.kind(), format!("`{tool}` not found; install ffmpeg"))
    } else {
        err
    }
}

/// Reads a stream's size and frame rate.
pub fn probe(path: &Path) -> io::Result<Info> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=width,height,avg_frame_rate,r_frame_rate"])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(path)
        .output()
        .map_err(|e| missing("ffprobe", e))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or("")
    };
    let rate = |s: &str| {
        let (n, d) = s.split_once('/').unwrap_or((s, "1"));
        let (n, d) = (n.parse::<f64>().ok()?, d.parse::<f64>().ok()?);
        (n > 0.0 && d > 0.0).then(|| n / d)
    };
    let (Ok(width), Ok(height)) = (field("width").parse(), field("height").parse()) else {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "{}: no video stream ({})",
            path.display(),
            err.trim()
        )));
    };
    let fps = rate(field("avg_frame_rate"))
        .or_else(|| rate(field("r_frame_rate")))
        .unwrap_or(25.0);
    Ok(Info { width, height, fps })
}

/// The largest size within `max_w × max_h` with the picture's proportions,
/// never larger than the picture itself, in even numbers.
pub fn fit_within(info: &Info, max_w: usize, max_h: usize) -> (usize, usize) {
    let scale = (max_w as f64 / info.width as f64)
        .min(max_h as f64 / info.height as f64)
        .min(1.0);
    let even = |v: f64| ((v / 2.0).round() as usize).max(1) * 2;
    (even(info.width as f64 * scale), even(info.height as f64 * scale))
}

/// A running `ffmpeg` writing raw RGB frames.
pub struct Decoder {
    child: Child,
    stdout: ChildStdout,
    pub width: usize,
    pub height: usize,
}

impl Decoder {
    /// Decodes `path` at `width × height` and a constant `fps`, from `start`
    /// seconds in. Transparency becomes black.
    pub fn spawn(path: &Path, width: usize, height: usize, fps: f64, start: f64) -> io::Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-v", "error", "-nostdin"]);
        if start > 0.0 {
            cmd.args(["-ss", &start.to_string()]);
        }
        let filters = format!("fps={fps},scale={width}:{height}:flags=area,format=gbrap,premultiply=inplace=1");
        cmd.arg("-i")
            .arg(path)
            .args([
                "-an", "-sn", "-vf", &filters, "-f", "rawvideo", "-pix_fmt", "rgb24", "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| missing("ffmpeg", e))?;
        let stdout = child.stdout.take().expect("piped stdout");
        Ok(Self {
            child,
            stdout,
            width,
            height,
        })
    }

    /// Reads the next frame into `buf`, returning `false` at the end.
    pub fn read(&mut self, buf: &mut Vec<u8>) -> io::Result<bool> {
        buf.resize(self.width * self.height * 3, 0);
        match self.stdout.read_exact(buf) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
            Err(e) => Err(e),
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Most decoded video kept in memory for replaying a loop without decoding
/// it again.
const CACHE_BYTES: usize = 256 << 20;

/// Decodes `path` on a background thread and sends its frames, `width ×
/// height` RGB, at the pace they are received. With `looping` it starts over
/// at the end: from memory when the whole file fit in the cache (GIFs, short
/// clips, stills), otherwise by decoding again. The channel closes at the
/// end, or straight away if nothing could be decoded.
pub fn frames(path: &Path, width: usize, height: usize, fps: f64, start: f64, looping: bool) -> Receiver<Arc<Vec<u8>>> {
    let (tx, rx) = mpsc::sync_channel(2);
    let path: PathBuf = path.into();
    thread::spawn(move || {
        let mut cache = Some(Vec::new());
        loop {
            let Ok(mut decoder) = Decoder::spawn(&path, width, height, fps, start) else {
                return;
            };
            let mut decoded = 0;
            loop {
                let mut buf = Vec::new();
                if !matches!(decoder.read(&mut buf), Ok(true)) {
                    break;
                }
                let frame = Arc::new(buf);
                decoded += 1;
                if let Some(frames) = &mut cache {
                    if (frames.len() + 1) * frame.len() <= CACHE_BYTES {
                        frames.push(Arc::clone(&frame));
                    } else {
                        cache = None;
                    }
                }
                if tx.send(frame).is_err() {
                    return;
                }
            }
            if decoded == 0 || !looping {
                return;
            }
            if let Some(frames) = cache.take() {
                for frame in frames.iter().cycle() {
                    if tx.send(Arc::clone(frame)).is_err() {
                        return;
                    }
                }
            }
        }
    });
    rx
}
