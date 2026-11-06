//! Connection check for the DSO2D15: `cargo run --example probe`.
//! Prints each step with its duration, the settings, and one two-channel record.

use std::time::Instant;

use husbb::measure::{measure, si, si_opt};
use husbb::scope::{self, Scope, Settings};
use husbb::visa::ResourceManager;

fn main() {
    let t = Instant::now();
    let step = |what: &str| println!("[{:6.0} ms] {what}", t.elapsed().as_secs_f64() * 1000.0);
    let rm = match ResourceManager::new() {
        Ok(rm) => rm,
        Err(e) => return println!("VISA library: {e}"),
    };
    step("VISA library loaded");
    match scope::find(&rm) {
        Ok(list) => step(&format!("found {list:?}")),
        Err(e) => return println!("find failed: {e}"),
    }
    let mut sc = match Scope::connect(&rm, None) {
        Ok(sc) => sc,
        Err(e) => return println!("connect failed after {:.1} s: {e}", t.elapsed().as_secs_f64()),
    };
    step(&format!("connected: {}", sc.idn));
    let mut s = Settings::default();
    if let Err(e) = sc.poll_all(&mut s) {
        return println!("settings failed: {e}");
    }
    step(&format!("settings: {s:?}"));
    for _ in 0..3 {
        match sc.read_display() {
            Ok(Some(acq)) => {
                step(&format!("record: channels {:?}, {} points", acq.header.enabled_channels().collect::<Vec<_>>(), acq.points()));
                for tr in &acq.traces {
                    let v = acq.volts(tr.channel).unwrap_or_default();
                    if let Some(m) = measure(&v, acq.header.sample_rate, acq.volts_per_count(tr.channel)) {
                        println!("           CH{}: f {}  Vpp {}  Vavg {}", tr.channel, si_opt(m.freq, "Hz"), si(m.vpp, "V"), si(m.vavg, "V"));
                    }
                }
            }
            Ok(None) => step("no display record yet"),
            Err(e) => step(&format!("read failed: {e}")),
        }
    }
}
