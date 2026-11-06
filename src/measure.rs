//! Automatic measurements on one channel record, computed on the PC.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurements {
    pub vmax: f64,
    pub vmin: f64,
    pub vpp: f64,
    pub vavg: f64,
    pub vrms: f64,
    /// `None` when fewer than two full periods are in the record, or the trace is flat.
    pub freq: Option<f64>,
    pub period: Option<f64>,
    /// Positive duty cycle in percent.
    pub duty: Option<f64>,
}

/// `lsb` is the voltage of one ADC count: a trace swinging less than 5 counts is treated as flat,
/// since ADC noise alone spans 2-3 counts.
pub fn measure(volts: &[f64], sample_rate: f64, lsb: f64) -> Option<Measurements> {
    if volts.is_empty() {
        return None;
    }
    let (mut vmax, mut vmin, mut sum, mut sq) = (f64::MIN, f64::MAX, 0.0, 0.0);
    for &v in volts {
        vmax = vmax.max(v);
        vmin = vmin.min(v);
        sum += v;
        sq += v * v;
    }
    let n = volts.len() as f64;
    let vpp = vmax - vmin;
    let (mut freq, mut period, mut duty) = (None, None, None);
    if vpp > 0.0 && vpp >= 5.0 * lsb {
        // Edges through the mid level with hysteresis, so noise near the level is not counted.
        let mid = (vmax + vmin) / 2.0;
        let hyst = (0.1 * vpp).max(1.5 * lsb);
        let (mut high, mut rising, mut falling) = (None::<bool>, Vec::new(), Vec::new());
        for (i, &v) in volts.iter().enumerate() {
            if v > mid + hyst && high != Some(true) {
                if high == Some(false) {
                    rising.push(i);
                }
                high = Some(true);
            } else if v < mid - hyst && high != Some(false) {
                if high == Some(true) {
                    falling.push(i);
                }
                high = Some(false);
            }
        }
        if rising.len() >= 2 {
            let span = (rising[rising.len() - 1] - rising[0]) as f64;
            let p = span / (rising.len() - 1) as f64 / sample_rate;
            period = Some(p);
            freq = Some(1.0 / p);
            // high time: from each rising edge to the next falling edge
            let highs: Vec<f64> = rising
                .iter()
                .filter_map(|&r| falling.iter().find(|&&f| f > r).map(|&f| (f - r) as f64))
                .take(rising.len() - 1)
                .collect();
            if !highs.is_empty() {
                let mean_high = highs.iter().sum::<f64>() / highs.len() as f64 / sample_rate;
                duty = Some(100.0 * mean_high / p);
            }
        }
    }
    Some(Measurements { vmax, vmin, vpp, vavg: sum / n, vrms: (sq / n).sqrt(), freq, period, duty })
}

const PREFIXES: [(f64, &str); 7] =
    [(1e9, "G"), (1e6, "M"), (1e3, "k"), (1.0, ""), (1e-3, "m"), (1e-6, "µ"), (1e-9, "n")];

/// Format with an SI prefix and 3 significant digits, e.g. `si(0.0002, "V") == "200mV"`.
pub fn si(value: f64, unit: &str) -> String {
    if value == 0.0 {
        return format!("0{unit}");
    }
    if !value.is_finite() {
        return format!("--{unit}");
    }
    let (scale, prefix) = PREFIXES
        .iter()
        .copied()
        .find(|(s, _)| value.abs() >= s * 0.9995)
        .unwrap_or(PREFIXES[PREFIXES.len() - 1]);
    let x = value / scale;
    let digits = if x.abs() >= 99.95 { 0 } else if x.abs() >= 9.995 { 1 } else { 2 };
    let s = format!("{x:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    format!("{s}{prefix}{unit}")
}

pub fn si_opt(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(|| "--".to_string(), |v| si(v, unit))
}

/// Next value up (+1) or down (-1) in the 1-2-5 sequence the scope's knobs use.
pub fn step_125(value: f64, direction: i32) -> f64 {
    let ladder: Vec<f64> = (-9..4).flat_map(|e| [1.0, 2.0, 5.0].map(|m| m * 10f64.powi(e))).collect();
    let nearest = ladder
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let da = (a.1 / value.max(1e-12)).ln().abs();
            let db = (b.1 / value.max(1e-12)).ln().abs();
            da.total_cmp(&db)
        })
        .map(|(i, _)| i)
        .unwrap_or(0) as i32;
    ladder[(nearest + direction).clamp(0, ladder.len() as i32 - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(freq: f64, rate: f64, n: usize, hi: f64, lo: f64, duty: f64) -> Vec<f64> {
        (0..n).map(|i| if (i as f64 / rate * freq).fract() < duty { hi } else { lo }).collect()
    }

    #[test]
    fn square_wave() {
        let m = measure(&square(1000.0, 1e6, 5000, 2.5, -0.5, 0.5), 1e6, 0.01).unwrap();
        assert!((m.freq.unwrap() - 1000.0).abs() < 1.0);
        assert!((m.vpp - 3.0).abs() < 1e-9);
        assert!((m.duty.unwrap() - 50.0).abs() < 0.5);
    }

    #[test]
    fn duty_cycle() {
        let m = measure(&square(1000.0, 1e6, 5000, 1.0, 0.0, 0.25), 1e6, 0.01).unwrap();
        assert!((m.duty.unwrap() - 25.0).abs() < 0.5, "{:?}", m.duty);
    }

    #[test]
    fn noise_has_no_frequency() {
        let noise: Vec<f64> = (0..4000).map(|i| [0.0, 0.02, -0.02, 0.0][i % 4]).collect();
        assert_eq!(measure(&noise, 1e6, 0.02).unwrap().freq, None);
    }

    #[test]
    fn si_format() {
        assert_eq!(si(0.2, "V"), "200mV");
        assert_eq!(si(2.0, "V"), "2V");
        assert_eq!(si(2e-5, "s"), "20µs");
        assert_eq!(si(12.5e6, "Sa/s"), "12.5MSa/s");
        assert_eq!(si(0.0, "V"), "0V");
        assert_eq!(si(-0.0125, "V"), "-12.5mV");
        assert_eq!(si_opt(None, "Hz"), "--");
    }

    #[test]
    fn steps() {
        assert_eq!(step_125(2.0, 1), 5.0);
        assert_eq!(step_125(2.0, -1), 1.0);
        assert!((step_125(200e-6, -1) - 100e-6).abs() < 1e-15);
        assert!((step_125(199.9e-6, 1) - 500e-6).abs() < 1e-15);
    }
}
