//! Acquisition thread: owns the scope connection, applies commands between records, keeps the
//! settings fresh, and publishes the latest record. The UI never blocks on USB.
//!
//! The link can be switched off: the session is closed and nothing is sent to the scope. The
//! DSO2D15 locks its front panel on every USB command and unlocks it by itself once no command
//! has arrived for about 5 s (firmware `phoenix`, check at 0x9428c: 4999 ms), so switching the
//! link off hands the scope back to its own keys.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use crate::protocol::Acquisition;
use crate::scope::{Scope, ScopeError, Setting, Settings, Slope, Sweep};
use crate::visa::ResourceManager;

/// How long after the last USB command the scope unlocks its front panel.
pub const PANEL_UNLOCK_AFTER: Duration = Duration::from_millis(5000);

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Run,
    Stop,
    Single,
    AutoSetup,
    ChannelDisplay(usize, bool),
    ChannelScale(usize, f64),
    ChannelOffset(usize, f64),
    Timebase(f64),
    TriggerSource(usize),
    TriggerLevel(f64),
    TriggerSlope(Slope),
    TriggerSweep(Sweep),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Starting,
    /// The VISA library could not be loaded; retried every few seconds.
    NoVisa(String),
    Searching,
    Connecting,
    Connected { idn: String, resource: String },
    Lost(String),
    /// Link switched off: nothing is sent; `since` is when the last command went out.
    Manual { since: Instant },
}

impl Status {
    /// Time left until the scope's front panel unlocks, while the link is off.
    pub fn panel_unlock_in(&self) -> Option<Duration> {
        match self {
            Status::Manual { since } => Some(PANEL_UNLOCK_AFTER.saturating_sub(since.elapsed())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub acq: Arc<Acquisition>,
    pub settings: Settings,
    /// When the record content last changed (the scope hands out each record more than once).
    pub changed_at: Instant,
    pub seq: u64,
}

#[derive(Debug, Clone)]
pub struct Shared {
    pub frame: Option<Frame>,
    pub status: Status,
    pub records_per_s: f64,
    pub last_error: Option<String>,
}

pub struct Acquirer {
    tx: Sender<Command>,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    link: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Acquirer {
    pub fn spawn(resource: Option<String>, notify: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let shared = Arc::new(Mutex::new(Shared { frame: None, status: Status::Starting, records_per_s: 0.0, last_error: None }));
        let stop = Arc::new(AtomicBool::new(false));
        let link = Arc::new(AtomicBool::new(true));
        let worker = Worker { rx, shared: shared.clone(), stop: stop.clone(), link: link.clone(), resource, notify: Box::new(notify) };
        let handle = std::thread::Builder::new()
            .name("acquisition".into())
            .spawn(move || worker.run())
            .expect("spawn acquisition thread");
        Self { tx, shared, stop, link, handle: Some(handle) }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn shared(&self) -> Shared {
        self.shared.lock().expect("acquisition state").clone()
    }

    /// Link on: read the scope continuously. Link off: close the session and send nothing.
    pub fn set_link(&self, on: bool) {
        self.link.store(on, Ordering::Relaxed);
    }

    pub fn link(&self) -> bool {
        self.link.load(Ordering::Relaxed)
    }
}

impl Drop for Acquirer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Worker {
    rx: Receiver<Command>,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    link: Arc<AtomicBool>,
    resource: Option<String>,
    notify: Box<dyn Fn() + Send>,
}

enum End {
    Lost(String),
    LinkOff,
    Stopped,
}

impl Worker {
    fn set(&self, f: impl FnOnce(&mut Shared)) {
        f(&mut self.shared.lock().expect("acquisition state"));
        (self.notify)();
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn linked(&self) -> bool {
        self.link.load(Ordering::Relaxed)
    }

    fn sleep(&self, d: Duration) {
        let end = Instant::now() + d;
        while !self.stopped() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Link off: stay idle until it is switched back on.
    fn manual(&self) {
        let since = Instant::now();
        self.set(|s| s.status = Status::Manual { since });
        // commands given while off would lock the panel again; drop them
        while !self.stopped() && !self.linked() {
            while self.rx.try_recv().is_ok() {}
            self.sleep(Duration::from_millis(100));
        }
        if !self.stopped() {
            self.set(|s| s.status = Status::Searching);
        }
    }

    fn run(self) {
        while !self.stopped() {
            if !self.linked() {
                self.manual();
                continue;
            }
            let rm = match ResourceManager::new() {
                Ok(rm) => rm,
                Err(e) => {
                    self.set(|s| s.status = Status::NoVisa(e.to_string()));
                    self.sleep(Duration::from_secs(5));
                    continue;
                }
            };
            while !self.stopped() && self.linked() {
                self.set(|s| {
                    if matches!(s.status, Status::Starting | Status::Searching) {
                        s.status = Status::Connecting;
                    }
                });
                match Scope::connect(&rm, self.resource.as_deref()) {
                    Ok(scope) => {
                        let (idn, resource) = (scope.idn.clone(), scope.resource.clone());
                        self.set(|s| s.status = Status::Connected { idn, resource });
                        match self.session(scope) {
                            End::Stopped => return,
                            End::LinkOff => break,
                            End::Lost(reason) => self.set(|s| s.status = Status::Lost(reason)),
                        }
                    }
                    Err(ScopeError::NotFound) => self.set(|s| {
                        if !matches!(s.status, Status::Lost(_)) {
                            s.status = Status::Searching;
                        }
                    }),
                    Err(e) => self.set(|s| s.last_error = Some(e.to_string())),
                }
                self.sleep(Duration::from_secs(1));
            }
        }
    }

    /// Runs until the connection is lost, the link is switched off, or the app stops. The scope
    /// session is dropped (closed) on return.
    fn session(&self, mut scope: Scope) -> End {
        // Each query costs ~200 ms on the DSO2D15: read only what the first record needs, the rest
        // is refreshed round-robin, one setting per record.
        let mut settings = Settings::default();
        for s in [Setting::Timebase, Setting::XIncrement, Setting::XOrigin, Setting::TriggerSource, Setting::TriggerLevel] {
            if let Err(e) = scope.poll(s, &mut settings) {
                return End::Lost(e.to_string());
            }
        }
        // round-robin starts with what the startup poll above skipped
        let (mut seq, mut poll_i) = (0u64, 5usize);
        let mut last_rate = 0.0;
        let mut prev: Option<Arc<Acquisition>> = None;
        let mut changed_at = Instant::now();
        let (mut window_start, mut window_records) = (Instant::now(), 0u32);

        loop {
            if self.stopped() {
                return End::Stopped;
            }
            if !self.linked() {
                return End::LinkOff;
            }
            let mut refresh: Vec<Setting> = Vec::new();
            while let Ok(cmd) = self.rx.try_recv() {
                match execute(&mut scope, &cmd) {
                    Ok(()) => refresh.extend(affected(&cmd)),
                    Err(e) => self.set(|s| s.last_error = Some(format!("{cmd:?}: {e}"))),
                }
            }
            refresh.dedup();
            let step = (|| -> Result<Option<Arc<Acquisition>>, ScopeError> {
                for s in &refresh {
                    scope.poll(*s, &mut settings)?;
                }
                let Some(acq) = scope.read_display()? else { return Ok(None) };
                if acq.header.sample_rate != last_rate {
                    // the sample rate follows the timebase knob: refresh the time axis right away
                    last_rate = acq.header.sample_rate;
                    for s in [Setting::Timebase, Setting::XIncrement, Setting::XOrigin] {
                        scope.poll(s, &mut settings)?;
                    }
                } else {
                    scope.poll(Setting::ALL[poll_i % Setting::ALL.len()], &mut settings)?;
                    poll_i += 1;
                }
                Ok(Some(Arc::new(acq)))
            })();
            match step {
                Ok(Some(acq)) => {
                    if prev.as_deref().is_none_or(|p| p.traces != acq.traces) {
                        changed_at = Instant::now();
                    }
                    prev = Some(acq.clone());
                    seq += 1;
                    window_records += 1;
                    let elapsed = window_start.elapsed().as_secs_f64();
                    let rate = (elapsed >= 2.0).then(|| window_records as f64 / elapsed);
                    if rate.is_some() {
                        (window_start, window_records) = (Instant::now(), 0);
                    }
                    let frame = Frame { acq, settings: settings.clone(), changed_at, seq };
                    self.set(|s| {
                        s.frame = Some(frame);
                        if let Some(r) = rate {
                            s.records_per_s = r;
                        }
                    });
                }
                Ok(None) => self.sleep(Duration::from_millis(100)), // no display record yet
                Err(ScopeError::Timeout) => self.set(|s| s.last_error = Some("the scope did not answer; retrying".into())),
                Err(ScopeError::Protocol(e)) => self.set(|s| s.last_error = Some(e.to_string())),
                Err(e) => return End::Lost(e.to_string()),
            }
        }
    }
}

/// Settings to read back after a command. Channel on/off, V/div, offset and run state come with
/// every record, so they need no query.
fn affected(cmd: &Command) -> Vec<Setting> {
    match cmd {
        Command::AutoSetup => Setting::ALL.to_vec(),
        Command::Timebase(_) => vec![Setting::Timebase, Setting::XIncrement, Setting::XOrigin],
        Command::TriggerSource(_) => vec![Setting::TriggerSource, Setting::TriggerLevel],
        Command::TriggerLevel(_) => vec![Setting::TriggerLevel],
        Command::TriggerSlope(_) => vec![Setting::TriggerSlope],
        Command::TriggerSweep(_) | Command::Single => vec![Setting::TriggerSweep],
        Command::Run | Command::Stop | Command::ChannelDisplay(..) | Command::ChannelScale(..) | Command::ChannelOffset(..) => Vec::new(),
    }
}

fn execute(scope: &mut Scope, cmd: &Command) -> Result<(), ScopeError> {
    match *cmd {
        Command::Run => scope.run(),
        Command::Stop => scope.stop(),
        Command::Single => scope.single(),
        Command::AutoSetup => scope.auto_setup(),
        Command::ChannelDisplay(ch, on) => scope.set_channel_display(ch, on),
        Command::ChannelScale(ch, v) => scope.set_channel_scale(ch, v),
        Command::ChannelOffset(ch, v) => scope.set_channel_offset(ch, v),
        Command::Timebase(s) => scope.set_timebase(s),
        Command::TriggerSource(ch) => scope.set_trigger_source(ch),
        Command::TriggerLevel(v) => scope.set_trigger_level(v),
        Command::TriggerSlope(s) => scope.set_trigger_slope(s),
        Command::TriggerSweep(s) => scope.set_trigger_sweep(s),
    }
}
