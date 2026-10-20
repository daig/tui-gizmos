//! GIFs and video as shimmering braille stipple art, for
//! [Ratatui](https://ratatui.rs).
//!
//! [`Stipple`] renders one RGB frame ([`Picture`]) into a terminal area, in
//! layers:
//!
//! - **Base:** the picture darkened and posterised into a few hard-edged
//!   steps, as cell colours. Cells without marks split into two colours
//!   with block glyphs, for detail finer than a cell ([`Detail`]).
//! - **Dots:** braille dots over the brighter detail. Each dot takes one pure
//!   primary, chosen in proportion to the picture's channels, so colour
//!   reads as a speckle of red, green and blue rather than a blend.
//! - **Clip:** cells that are bright all over become solid blocks.
//!
//! The dots are dealt by a seed. Give each new video frame a new seed and
//! the dither is redone frame by frame, as in any dithered video; keep it
//! for a still picture and it stays still. [`Look`] holds every parameter.
//! Optional flourishes are off by default: [`Look::ACCENTED`] adds dashes
//! and drips, and [`Glitch`] adds datamosh-style glitches.
//!
//! The `stipple` binary (feature `player`, on by default) plays any file
//! `ffmpeg` can decode.

mod blocks;
mod glitch;
mod levels;
mod look;
mod noise;
mod picture;
mod render;

pub use blocks::Detail;
pub use glitch::Glitch;
pub use levels::{AutoLevels, Levels};
pub use look::{Look, Rgb};
pub use picture::{Fit, Picture};
pub use render::Stipple;
