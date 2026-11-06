//! Time axis and CSV export.

use std::io::Write;
use std::path::Path;

use crate::protocol::Acquisition;
use crate::scope::Settings;

/// Seconds of each sample relative to the trigger point. Uses the scope's own
/// `:WAVeform:XINCrement?` / `:WAVeform:XORigin?` when known, otherwise the sample rate with the
/// trigger in the middle of the record.
pub fn time_axis(acq: &Acquisition, settings: &Settings) -> Vec<f64> {
    let n = acq.points();
    let (dx, x0) = if settings.x_increment > 0.0 {
        (settings.x_increment, settings.x_origin)
    } else {
        let dx = 1.0 / acq.header.sample_rate;
        (dx, n as f64 / 2.0 * dx)
    };
    (0..n).map(|i| i as f64 * dx - x0).collect()
}

pub fn write_csv(path: &Path, acq: &Acquisition, settings: &Settings, channels: &[usize]) -> std::io::Result<()> {
    let t = time_axis(acq, settings);
    let cols: Vec<(usize, Vec<f64>)> = channels.iter().filter_map(|&ch| acq.volts(ch).map(|v| (ch, v))).collect();
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(f, "time_s")?;
    for (ch, _) in &cols {
        write!(f, ",ch{ch}_V")?;
    }
    writeln!(f)?;
    for (i, ti) in t.iter().enumerate() {
        write!(f, "{ti:.9e}")?;
        for (_, v) in &cols {
            write!(f, ",{:.6e}", v[i])?;
        }
        writeln!(f)?;
    }
    f.flush()
}
