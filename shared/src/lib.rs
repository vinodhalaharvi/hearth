//! hearth-shared — code reused across hearth's ESP32-family firmware.
//!
//! Two independent subsystems share this crate; cargo features keep each
//! firmware lean (a WiFi node never compiles NimBLE; a BLE node never compiles
//! the MQTT client):
//!
//! - [`command`] — structured command envelope / **BLE wire contract**. Pure,
//!                 always compiled, host-testable.
//! - [`ble`]     — reusable NimBLE command peripheral. Feature `ble`.
//! - [`mqtt`]    — WiFi/MQTT transport **and** on-the-wire contract (topics,
//!                 payloads, last-will). Feature `mqtt` (default).
//! - [`drivers`] — I2C/SPI sensor drivers shared across boards. Stub for now.
//! - [`manifest`]— node manifest → discovery/codegen tooling. Stub for now.
//!
//! Add API here only when a node actually needs it — no speculative surface.

pub mod command;

#[cfg(feature = "mqtt")]
pub mod mqtt;

#[cfg(feature = "ble")]
pub mod ble;

pub mod drivers;
pub mod manifest;
