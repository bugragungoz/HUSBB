//! A DSO2000-series scope over USBTMC: identification, settings, commands and display records.

use thiserror::Error;

use crate::protocol::{Acquisition, Assembler, Feed, ProtocolError};
use crate::visa::{ResourceManager, Session, VisaError};

pub const VID: u16 = 0x049F;
pub const PID: u16 = 0x505E;
const TIMEOUT_MS: u32 = 3000;
const CONNECT_TIMEOUT_MS: u32 = 1000;

#[derive(Debug, Error)]
pub enum ScopeError {
    #[error("no Hantek DSO2000 scope found on USB")]
    NotFound,
    #[error("the scope did not answer in time")]
    Timeout,
    #[error("connection lost: {0}")]
    Disconnected(String),
    #[error("VISA: {0}")]
    Visa(String),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("unexpected reply to {cmd}: {reply:?}")]
    BadReply { cmd: String, reply: String },
}

pub type Result<T> = std::result::Result<T, ScopeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slope {
    Rising,
    Falling,
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sweep {
    Auto,
    Normal,
    Single,
}

impl Sweep {
    pub const ALL: [Sweep; 3] = [Sweep::Auto, Sweep::Normal, Sweep::Single];

    pub fn label(self) -> &'static str {
        match self {
            Sweep::Auto => "Auto",
            Sweep::Normal => "Normal",
            Sweep::Single => "Single",
        }
    }

    fn scpi(self) -> &'static str {
        match self {
            Sweep::Auto => "AUTO",
            Sweep::Normal => "NORMal",
            Sweep::Single => "SINGle",
        }
    }
}

/// Settings that are not in the waveform header; each costs one query to refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub timebase: f64,
    pub x_increment: f64,
    pub x_origin: f64,
    pub trigger_source: usize,
    pub trigger_level: f64,
    pub trigger_slope: Slope,
    pub trigger_sweep: Sweep,
    pub probe: [f64; 2],
    pub coupling: [String; 2],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timebase: 0.0,
            x_increment: 0.0,
            x_origin: 0.0,
            trigger_source: 1,
            trigger_level: 0.0,
            trigger_slope: Slope::Rising,
            trigger_sweep: Sweep::Auto,
            probe: [0.0; 2], // unknown until polled
            coupling: [String::new(), String::new()],
        }
    }
}

/// One refreshable setting; the acquisition loop polls them round-robin, one per record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Timebase,
    XIncrement,
    XOrigin,
    TriggerSource,
    TriggerLevel,
    TriggerSlope,
    TriggerSweep,
    Probe(usize),
    Coupling(usize),
}

impl Setting {
    pub const ALL: [Setting; 11] = [
        Setting::Timebase,
        Setting::XIncrement,
        Setting::XOrigin,
        Setting::TriggerSource,
        Setting::TriggerLevel,
        Setting::TriggerSlope,
        Setting::TriggerSweep,
        Setting::Probe(1),
        Setting::Probe(2),
        Setting::Coupling(1),
        Setting::Coupling(2),
    ];
}

/// C `%.6e` formatting ("5.000000e-04"), the form the scope has been seen to accept.
fn sci(v: f64) -> String {
    let s = format!("{v:.6e}");
    match s.split_once('e') {
        Some((mantissa, exp)) => {
            let exp: i32 = exp.parse().unwrap_or(0);
            format!("{mantissa}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
        }
        None => s,
    }
}

pub fn find(rm: &ResourceManager) -> Result<Vec<String>> {
    let all = rm.find("USB?*INSTR").map_err(|e| ScopeError::Visa(e.to_string()))?;
    let tag = format!("0X{VID:04X}::0X{PID:04X}");
    let dec = format!("::{VID}::{PID}::");
    Ok(all.into_iter().filter(|r| r.to_uppercase().contains(&tag) || r.contains(&dec)).collect())
}

pub struct Scope {
    session: Option<Session>,
    assembler: Assembler,
    pub resource: String,
    pub idn: String,
}

impl Scope {
    /// Open the given resource, or the first scope found.
    pub fn connect(rm: &ResourceManager, resource: Option<&str>) -> Result<Self> {
        let resource = match resource {
            Some(r) => r.to_string(),
            None => find(rm)?.into_iter().next().ok_or(ScopeError::NotFound)?,
        };
        // A client that went away in the middle of a transfer leaves the scope's USBTMC side in a
        // state where the first exchange of the next session fails (measured: a timeout, or
        // VI_ERROR_IO); a fresh session then works. Use a short timeout while connecting so this
        // costs about a second instead of several.
        let mut last = ScopeError::NotFound;
        for _ in 0..3 {
            let session = rm.open(&resource, CONNECT_TIMEOUT_MS).map_err(|e| ScopeError::Disconnected(e.to_string()))?;
            let mut scope = Scope { session: Some(session), assembler: Assembler::default(), resource: resource.clone(), idn: String::new() };
            match scope.exchange_init() {
                Ok(()) => return Ok(scope),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn exchange_init(&mut self) -> Result<()> {
        let s = self.session()?;
        s.clear().map_err(|e| ScopeError::Disconnected(e.to_string()))?;
        self.idn = self.query("*IDN?")?;
        self.session()?.set_timeout(TIMEOUT_MS).map_err(|e| ScopeError::Disconnected(e.to_string()))?;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    fn session(&self) -> Result<&Session> {
        self.session.as_ref().ok_or_else(|| ScopeError::Disconnected("not connected".into()))
    }

    fn fail(&mut self, e: VisaError) -> ScopeError {
        match e {
            VisaError::Timeout => {
                // A query left unanswered has to be cleared, or the next read returns its late
                // reply (or nothing at all).
                self.assembler.reset();
                match self.session.as_ref().map(Session::clear) {
                    Some(Ok(())) => ScopeError::Timeout,
                    _ => {
                        self.session = None;
                        ScopeError::Disconnected("timeout, and the device clear failed".into())
                    }
                }
            }
            other => {
                self.session = None;
                ScopeError::Disconnected(other.to_string())
            }
        }
    }

    pub fn write(&mut self, cmd: &str) -> Result<()> {
        let r = self.session()?.write(format!("{cmd}\n").as_bytes());
        r.map_err(|e| self.fail(e))
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        let r = self.session()?.read();
        r.map_err(|e| self.fail(e))
    }

    pub fn query(&mut self, cmd: &str) -> Result<String> {
        self.write(cmd)?;
        let raw = self.read()?;
        Ok(String::from_utf8_lossy(&raw).trim().to_string())
    }

    pub fn query_f64(&mut self, cmd: &str) -> Result<f64> {
        let reply = self.query(cmd)?;
        reply.parse().map_err(|_| ScopeError::BadReply { cmd: cmd.into(), reply })
    }

    /// The record shown on screen, every channel that is on, from one acquisition.
    /// `Ok(None)` when the scope has no display record ready yet.
    pub fn read_display(&mut self) -> Result<Option<Acquisition>> {
        for _ in 0..16 {
            self.write(":WAVeform:DATA:DISP?")?;
            let raw = self.read()?;
            match self.assembler.feed(&raw) {
                Ok(Feed::Pending) => continue,
                Ok(Feed::NoSnapshot) => return Ok(None),
                Ok(Feed::Complete(acq)) => return Ok(Some(acq)),
                Err(e) => {
                    self.assembler.reset();
                    return Err(e.into());
                }
            }
        }
        Err(ScopeError::BadReply { cmd: ":WAVeform:DATA:DISP?".into(), reply: "no complete stream in 16 packets".into() })
    }

    pub fn poll(&mut self, setting: Setting, into: &mut Settings) -> Result<()> {
        match setting {
            Setting::Timebase => into.timebase = self.query_f64(":TIMebase:SCALe?")?,
            Setting::XIncrement => into.x_increment = self.query_f64(":WAVeform:XINCrement?")?,
            Setting::XOrigin => into.x_origin = self.query_f64(":WAVeform:XORigin?")?,
            Setting::TriggerSource => {
                let r = self.query(":TRIGger:EDGe:SOURce?")?;
                into.trigger_source = if r.trim_end().ends_with('2') { 2 } else { 1 };
            }
            Setting::TriggerLevel => into.trigger_level = self.query_f64(":TRIGger:EDGe:LEVel?")?,
            Setting::TriggerSlope => {
                let r = self.query(":TRIGger:EDGe:SLOPe?")?.to_uppercase();
                into.trigger_slope = if r.starts_with("FALL") {
                    Slope::Falling
                } else if r.starts_with("RIS") {
                    Slope::Rising
                } else {
                    Slope::Either
                };
            }
            Setting::TriggerSweep => {
                let r = self.query(":TRIGger:SWEep?")?.to_uppercase();
                into.trigger_sweep = if r.starts_with("SING") {
                    Sweep::Single
                } else if r.starts_with("NORM") {
                    Sweep::Normal
                } else {
                    Sweep::Auto
                };
            }
            Setting::Probe(ch) => into.probe[ch - 1] = self.query_f64(&format!(":CHANnel{ch}:PROBe?"))?,
            Setting::Coupling(ch) => into.coupling[ch - 1] = self.query(&format!(":CHANnel{ch}:COUPling?"))?,
        }
        Ok(())
    }

    pub fn poll_all(&mut self, into: &mut Settings) -> Result<()> {
        for s in Setting::ALL {
            self.poll(s, into)?;
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        self.write(":RUNning ON")
    }

    pub fn stop(&mut self) -> Result<()> {
        self.write(":RUNning OFF")
    }

    /// `:SINGle` is accepted but does nothing on firmware 3.0.1; Hantek's own software sends this.
    pub fn single(&mut self) -> Result<()> {
        self.write(":TRIGger:SWEep SINGle")
    }

    pub fn auto_setup(&mut self) -> Result<()> {
        self.write(":AUToscale")
    }

    pub fn set_channel_display(&mut self, ch: usize, on: bool) -> Result<()> {
        self.write(&format!(":CHANnel{ch}:DISPlay {}", if on { "ON" } else { "OFF" }))
    }

    /// Volts/div at the probe tip.
    pub fn set_channel_scale(&mut self, ch: usize, volts_per_div: f64) -> Result<()> {
        self.write(&format!(":CHANnel{ch}:SCALe {}", sci(volts_per_div)))
    }

    /// Vertical offset in volts at the probe tip.
    pub fn set_channel_offset(&mut self, ch: usize, volts: f64) -> Result<()> {
        self.write(&format!(":CHANnel{ch}:OFFSet {}", sci(volts)))
    }

    pub fn set_timebase(&mut self, seconds_per_div: f64) -> Result<()> {
        self.write(&format!(":TIMebase:SCALe {}", sci(seconds_per_div)))
    }

    pub fn set_trigger_source(&mut self, ch: usize) -> Result<()> {
        self.write(&format!(":TRIGger:EDGe:SOURce CHANnel{ch}"))
    }

    pub fn set_trigger_level(&mut self, volts: f64) -> Result<()> {
        self.write(&format!(":TRIGger:EDGe:LEVel {}", sci(volts)))
    }

    pub fn set_trigger_slope(&mut self, slope: Slope) -> Result<()> {
        let s = match slope {
            Slope::Rising => "RISIng",
            Slope::Falling => "FALLing",
            Slope::Either => "EITHer",
        };
        self.write(&format!(":TRIGger:EDGe:SLOPe {s}"))
    }

    pub fn set_trigger_sweep(&mut self, sweep: Sweep) -> Result<()> {
        self.write(&format!(":TRIGger:SWEep {}", sweep.scpi()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sci_matches_c() {
        assert_eq!(super::sci(5e-4), "5.000000e-04");
        assert_eq!(super::sci(2.5), "2.500000e+00");
        assert_eq!(super::sci(-20.0), "-2.000000e+01");
    }
}
