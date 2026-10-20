//! Graphical gizmos for [Ratatui](https://ratatui.rs).
//!
//! | Gizmo | What it shows |
//! |-------|---------------|
//! | [`Speedometer`] | A braille dial with needle, gradient arc, ticks and readout |
//! | [`ArcSpeedometer`] | A two-row lit arc with the readout inside it |
//! | [`MiniSpeedometer`] | A three-line half dial with the readout alongside |
//!
//! Every gizmo is a stateless [`Widget`](ratatui::widgets::Widget): build it
//! each frame from your current values. The [`Damped`] and [`PeakHold`]
//! helpers are there for when you want motion that feels physical.
//!
//! ```no_run
//! use tui_gizmos::Speedometer;
//!
//! # fn draw(frame: &mut ratatui::Frame, rate: f64) {
//! frame.render_widget(Speedometer::bytes_per_sec(rate), frame.area());
//! # }
//! ```

mod arc;
mod braille;
mod dial;
pub mod format;
mod macros;
mod mini;
mod motion;
mod speedometer;

pub use arc::ArcSpeedometer;
pub use dial::{Fill, Scale};
pub use mini::MiniSpeedometer;
pub use motion::{Damped, PeakHold};
pub use speedometer::Speedometer;
