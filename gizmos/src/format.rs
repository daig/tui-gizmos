//! Compact human-readable number formatting for gizmo readouts.

const BYTE_UNITS: [&str; 7] = ["B", "kB", "MB", "GB", "TB", "PB", "EB"];
const SI_SUFFIXES: [&str; 7] = ["", "k", "M", "G", "T", "P", "E"];

/// Scales `value` by powers of 1000 and returns the mantissa (3 significant
/// digits) and the index of the unit it was scaled to.
fn scale(value: f64) -> (String, usize) {
    if !value.is_finite() {
        return ("–".into(), 0);
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let mut v = value.abs();
    let mut unit = 0;
    // Bump the unit when rounding would print "1000".
    while v >= 999.5 && unit + 1 < BYTE_UNITS.len() {
        v /= 1000.0;
        unit += 1;
    }
    let n = match v {
        // Whole units (bytes, plain counts) never have a fractional part.
        v if unit == 0 && v.fract() == 0.0 => format!("{v:.0}"),
        v if v < 9.995 => format!("{v:.2}"),
        v if v < 99.95 => format!("{v:.1}"),
        v => format!("{v:.0}"),
    };
    (format!("{sign}{n}"), unit)
}

/// Formats a byte count with decimal (SI) units, e.g. `18.6 GB`.
///
/// ```
/// assert_eq!(tui_gizmos::format::bytes(18_600_000_000.0), "18.6 GB");
/// assert_eq!(tui_gizmos::format::bytes(512.0), "512 B");
/// assert_eq!(tui_gizmos::format::bytes(0.0), "0 B");
/// ```
#[must_use]
pub fn bytes(value: f64) -> String {
    let (n, unit) = scale(value);
    format!("{n} {}", BYTE_UNITS[unit])
}

/// Formats a transfer rate, e.g. `49.6 MB/s`.
///
/// ```
/// assert_eq!(tui_gizmos::format::bytes_per_sec(49_600_000.0), "49.6 MB/s");
/// ```
#[must_use]
pub fn bytes_per_sec(value: f64) -> String {
    format!("{}/s", bytes(value))
}

/// Formats a number with a bare SI suffix and no padding, e.g. `1G`, `250k`.
/// Handy for short scale labels.
///
/// ```
/// assert_eq!(tui_gizmos::format::si(1e9), "1G");
/// assert_eq!(tui_gizmos::format::si(250_000.0), "250k");
/// ```
#[must_use]
pub fn si(value: f64) -> String {
    let (n, unit) = scale(value);
    let n = if n.contains('.') {
        n.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        n
    };
    format!("{n}{}", SI_SUFFIXES[unit])
}
