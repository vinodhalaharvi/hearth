//! hearth-shared — code reused across hearth's ESP32-family firmware nodes.
//!
//! Keep this crate minimal and *real*. The modules below are the seams the
//! whole system leans on:
//!
//! - [`mqtt`]     — the transport **and** the on-the-wire contract (topics,
//!                  payloads, last-will). This is the important one; implemented.
//! - [`drivers`]  — I2C/SPI sensor drivers shared across boards. Stub for now.
//! - [`manifest`] — node manifest → discovery/codegen tooling. Stub for now.
//!
//! Add API here only when a node actually needs it — no speculative surface.

pub mod mqtt;
pub mod drivers;
pub mod manifest;
