//! HUSBB - Hantek DSO2D15 USB Bridge: protocol, VISA session and acquisition for the DSO2D15.
//!
//! Both channels are read from one acquisition through `:WAVeform:DATA:DISP?`, a command found in
//! the DSO2D15's firmware; see [`protocol`] and `docs/PROTOCOL.md`.

pub mod acquire;
pub mod export;
pub mod measure;
pub mod protocol;
pub mod scope;
pub mod visa;
