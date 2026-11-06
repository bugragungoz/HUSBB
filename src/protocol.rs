//! `:WAVeform:DATA:DISP?` stream of Hantek DSO2000-series scopes (firmware 3.0.1).
//!
//! Layout recovered from the scope's own firmware (`phoenix`, handler at 0x848d0) and checked
//! against the hardware. One query returns one packet; a stream is
//!
//! * a 128-byte header packet:
//!   `#9` `%09d` length (=128) `%01d` running `%01d` triggered `%09d` total `%09d` position
//!   `%04d`×4 channel offsets (screen counts) `%.1e`×4 volts/div (probe applied)
//!   `%01d`×4 channel on `%.3e` sample rate `%06d` … (rest unused here)
//! * then one packet per channel that is on, in channel order:
//!   `#9` `%09d` length `%09d` total `%09d` position, followed by the samples.
//!
//! The samples are the record shown on screen (3850 points at the default 4K memory depth), as
//! signed 8-bit screen positions: 0 is mid-screen, 25 counts per division, offset included.
//! Unlike `:WAVeform:DATA:ALL?`, which sends one channel twice when both are on, every channel
//! here comes from its own buffer of the same acquisition.
//!
//! When the scope has no display record to give, it answers with a bare `#9000000000`.

use thiserror::Error;

pub const COUNTS_PER_DIV: f64 = 25.0;
pub const HEADER_LEN: usize = 128;
const PACKET_HEADER_LEN: usize = 29;
const NO_SNAPSHOT: &[u8] = b"#9000000000";

#[derive(Debug, Error, PartialEq)]
pub enum ProtocolError {
    #[error("not a waveform packet: {0:?}")]
    NotAPacket(String),
    #[error("malformed header field {field}: {text:?}")]
    BadField { field: &'static str, text: String },
    #[error("packet at {pos} overruns the {total}-byte stream")]
    Overrun { pos: usize, total: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub running: bool,
    pub triggered: bool,
    /// Bytes in the whole stream: 99 + samples of all channels.
    pub total: usize,
    /// Channel offsets in screen counts (25 per division), CH1..CH4.
    pub offsets: [i32; 4],
    /// Volts per division at the probe tip, CH1..CH4.
    pub volts_per_div: [f64; 4],
    pub enabled: [bool; 4],
    pub sample_rate: f64,
}

impl Header {
    pub fn enabled_channels(&self) -> impl Iterator<Item = usize> + '_ {
        (0..4).filter(|&i| self.enabled[i]).map(|i| i + 1)
    }
}

fn field<'a>(raw: &'a [u8], range: std::ops::Range<usize>, name: &'static str) -> Result<&'a str, ProtocolError> {
    raw.get(range)
        .and_then(|b| std::str::from_utf8(b).ok())
        .ok_or_else(|| ProtocolError::BadField { field: name, text: String::from_utf8_lossy(raw).into() })
}

fn number<T: std::str::FromStr>(raw: &[u8], range: std::ops::Range<usize>, name: &'static str) -> Result<T, ProtocolError> {
    let text = field(raw, range, name)?;
    text.trim()
        .parse()
        .map_err(|_| ProtocolError::BadField { field: name, text: text.into() })
}

pub fn parse_header(raw: &[u8]) -> Result<Header, ProtocolError> {
    if raw.len() < HEADER_LEN || !raw.starts_with(b"#9000000128") {
        return Err(ProtocolError::NotAPacket(String::from_utf8_lossy(&raw[..raw.len().min(16)]).into()));
    }
    let flag = |i: usize| raw[i] == b'1';
    let mut offsets = [0; 4];
    let mut volts_per_div = [0.0; 4];
    let mut enabled = [false; 4];
    for ch in 0..4 {
        offsets[ch] = number(raw, 31 + 4 * ch..35 + 4 * ch, "offset")?;
        volts_per_div[ch] = number(raw, 47 + 7 * ch..54 + 7 * ch, "volts/div")?;
        enabled[ch] = flag(75 + ch);
    }
    Ok(Header {
        running: flag(11),
        triggered: flag(12),
        total: number(raw, 13..22, "total")?,
        offsets,
        volts_per_div,
        enabled,
        sample_rate: number(raw, 79..88, "sample rate")?,
    })
}

/// One channel's record: signed screen counts.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub channel: usize,
    pub samples: Vec<i8>,
}

/// One acquisition as the scope displays it: all channels that are on, captured together.
#[derive(Debug, Clone, PartialEq)]
pub struct Acquisition {
    pub header: Header,
    pub traces: Vec<Trace>,
}

impl Acquisition {
    pub fn trace(&self, channel: usize) -> Option<&Trace> {
        self.traces.iter().find(|t| t.channel == channel)
    }

    pub fn points(&self) -> usize {
        self.traces.first().map_or(0, |t| t.samples.len())
    }

    /// Volts per ADC count for a channel.
    pub fn volts_per_count(&self, channel: usize) -> f64 {
        self.header.volts_per_div[channel - 1] / COUNTS_PER_DIV
    }

    /// Samples converted to volts at the probe tip.
    pub fn volts(&self, channel: usize) -> Option<Vec<f64>> {
        let t = self.trace(channel)?;
        let off = self.header.offsets[channel - 1] as f64;
        let k = self.volts_per_count(channel);
        Some(t.samples.iter().map(|&s| (s as f64 - off) * k).collect())
    }

    /// Vertical screen position in divisions, exactly as the scope draws it.
    pub fn divisions(&self, channel: usize) -> Option<impl Iterator<Item = f64> + '_> {
        Some(self.trace(channel)?.samples.iter().map(|&s| s as f64 / COUNTS_PER_DIV))
    }
}

#[derive(Debug, PartialEq)]
pub enum Feed {
    /// More packets needed.
    Pending,
    /// The scope had nothing to send; ask again a little later.
    NoSnapshot,
    Complete(Acquisition),
}

/// Assembles packets, in arrival order, into acquisitions. Packets that arrive before a header
/// (a reader that joined mid-stream) are dropped, since the scope keeps its cursor between calls.
#[derive(Default)]
pub struct Assembler {
    header: Option<Header>,
    traces: Vec<Trace>,
    received: usize,
}

impl Assembler {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn feed(&mut self, raw: &[u8]) -> Result<Feed, ProtocolError> {
        if raw.starts_with(NO_SNAPSHOT) && raw.len() <= NO_SNAPSHOT.len() + 2 {
            self.reset();
            return Ok(Feed::NoSnapshot);
        }
        if raw.starts_with(b"#9000000128") {
            let header = parse_header(raw)?;
            self.reset();
            self.received = 99;
            self.header = Some(header);
            return Ok(Feed::Pending);
        }
        if raw.len() < PACKET_HEADER_LEN || !raw.starts_with(b"#9") {
            return Err(ProtocolError::NotAPacket(String::from_utf8_lossy(&raw[..raw.len().min(16)]).into()));
        }
        let Some(header) = &self.header else {
            return Ok(Feed::Pending); // joined mid-stream: wait for the next header
        };
        let total: usize = number(raw, 11..20, "total")?;
        let pos: usize = number(raw, 20..29, "position")?;
        let samples: Vec<i8> = raw[PACKET_HEADER_LEN..].iter().map(|&b| b as i8).collect();
        if total != header.total || pos + samples.len() > total {
            let total = header.total;
            self.reset();
            return Err(ProtocolError::Overrun { pos, total });
        }
        let channel = header
            .enabled_channels()
            .nth(self.traces.len())
            .ok_or(ProtocolError::Overrun { pos, total })?;
        self.received = pos + samples.len();
        self.traces.push(Trace { channel, samples });
        if self.received < total {
            return Ok(Feed::Pending);
        }
        let header = self.header.take().expect("checked above");
        let traces = std::mem::take(&mut self.traces);
        self.reset();
        Ok(Feed::Complete(Acquisition { header, traces }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: &[u8] = include_bytes!("../tests/data/disp_both_0.bin");
    const CH1: &[u8] = include_bytes!("../tests/data/disp_both_1.bin");
    const CH2: &[u8] = include_bytes!("../tests/data/disp_both_2.bin");

    #[test]
    fn header_matches_scope_settings() {
        // recorded with CH1 0.5 V/div and CH2 5 V/div (10x probes), offsets 0, 200 us/div
        let h = parse_header(HDR).unwrap();
        assert!(h.running);
        assert_eq!(h.total, 99 + 2 * 3850);
        assert_eq!(h.offsets, [0, 0, 0, 0]);
        assert_eq!(&h.volts_per_div[..2], &[0.5, 5.0]);
        assert_eq!(h.enabled, [true, true, false, false]);
        assert_eq!(h.sample_rate, 1.25e6);
    }

    #[test]
    fn assembles_both_channels_from_one_stream() {
        let mut a = Assembler::default();
        assert_eq!(a.feed(HDR).unwrap(), Feed::Pending);
        assert_eq!(a.feed(CH1).unwrap(), Feed::Pending);
        let Feed::Complete(acq) = a.feed(CH2).unwrap() else { panic!("expected a complete acquisition") };
        assert_eq!(acq.points(), 3850);
        let ch1 = &acq.trace(1).unwrap().samples;
        let ch2 = &acq.trace(2).unwrap().samples;
        // CH1 was open (noise around 0), CH2 on the 1 kHz probe-compensation square wave
        assert!(ch1.iter().all(|&s| (-4..=4).contains(&s)));
        assert!(*ch2.iter().max().unwrap() > 20 && *ch2.iter().min().unwrap() < 3);
    }

    #[test]
    fn packets_before_a_header_are_ignored() {
        let mut a = Assembler::default();
        assert_eq!(a.feed(CH2).unwrap(), Feed::Pending);
        assert_eq!(a.feed(HDR).unwrap(), Feed::Pending);
        assert_eq!(a.feed(CH1).unwrap(), Feed::Pending);
        assert!(matches!(a.feed(CH2).unwrap(), Feed::Complete(_)));
    }

    #[test]
    fn no_snapshot_marker() {
        let mut a = Assembler::default();
        a.feed(HDR).unwrap();
        assert_eq!(a.feed(b"#9000000000").unwrap(), Feed::NoSnapshot);
        // the half-built stream is dropped
        assert_eq!(a.feed(CH1).unwrap(), Feed::Pending);
    }

    #[test]
    fn rejects_garbage() {
        let mut a = Assembler::default();
        assert!(a.feed(b"1.000e+00\n").is_err());
        assert!(parse_header(b"#9000000128 too short").is_err());
    }

    #[test]
    fn volts_use_offset_and_scale() {
        let mut a = Assembler::default();
        a.feed(HDR).unwrap();
        a.feed(CH1).unwrap();
        let Feed::Complete(acq) = a.feed(CH2).unwrap() else { panic!() };
        let v = acq.volts(2).unwrap();
        let max = v.iter().cloned().fold(f64::MIN, f64::max);
        // 24-25 counts at 5 V/div = 4.8-5.0 V (the scope measured Vmax 5.12 V on this signal at 2 V/div)
        assert!((4.6..=5.2).contains(&max), "{max}");
    }
}
