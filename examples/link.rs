//! Live-link check on the hardware: `cargo run --example link`.
//! Connects, switches the link off (the session closes and nothing is sent), waits, switches it
//! back on, and prints the status at each step.

use std::time::{Duration, Instant};

use husbb::acquire::{Acquirer, Status};

fn wait(acq: &Acquirer, what: &str, until: impl Fn(&Status) -> bool) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(20) {
        let st = acq.shared().status;
        if until(&st) {
            println!("{what}: {st:?} after {:.1} s", t.elapsed().as_secs_f64());
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("{what}: timed out, status {:?}", acq.shared().status);
}

fn main() {
    let acq = Acquirer::spawn(None, || {});
    wait(&acq, "connected", |s| matches!(s, Status::Connected { .. }));
    std::thread::sleep(Duration::from_secs(2));
    let seq = acq.shared().frame.map(|f| f.seq).unwrap_or(0);
    println!("records so far: {seq}");
    acq.set_link(false);
    wait(&acq, "link off", |s| matches!(s, Status::Manual { .. }));
    std::thread::sleep(Duration::from_secs(6));
    let st = acq.shared().status;
    println!("6 s later: {st:?}, panel unlock in {:?}", st.panel_unlock_in());
    let seq_off = acq.shared().frame.map(|f| f.seq).unwrap_or(0);
    // the read in flight when the link went off may still complete; nothing after it
    println!("records finished after switching off: {} (0 or 1)", seq_off.saturating_sub(seq));
    acq.set_link(true);
    wait(&acq, "link on again", |s| matches!(s, Status::Connected { .. }));
    std::thread::sleep(Duration::from_secs(2));
    println!("records after re-link: {}", acq.shared().frame.map(|f| f.seq).unwrap_or(0));
}
