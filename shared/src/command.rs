//! Structured command envelope — the reusable heart of BLE control.
//!
//! **This is the wire contract** (the BLE analog of the MQTT payload contract):
//! the exact bytes a client writes to the command characteristic. It does NOT
//! depend on Rust struct memory layout — it is an explicit byte format:
//!
//! ```text
//!   byte 0      = command id (u8)
//!   bytes 1..N  = payload, meaning depends on the id
//! ```
//!
//! | id   | command        | payload            |
//! |------|----------------|--------------------|
//! | 0x00 | LedOff         | (none)             |
//! | 0x01 | LedOn          | (none)             |
//! | 0x02 | SetColor       | 3 bytes: R, G, B   |
//! | 0x03 | SetBrightness  | 1 byte: 0..=255    |
//!
//! A future actuator is a new variant + id + a few payload bytes; the BLE
//! plumbing (`ble.rs`) never changes — only `decode`/`encode` and the handler.

use core::fmt;

/// Command ids as written in byte 0. Named constants so the wire format is
/// documented in one place and shared by decode/encode.
pub mod id {
    pub const LED_OFF: u8 = 0x00;
    pub const LED_ON: u8 = 0x01;
    pub const SET_COLOR: u8 = 0x02;
    pub const SET_BRIGHTNESS: u8 = 0x03;
    // Future, e.g. a motor: SET_THROTTLE = 0x10 (payload: i16 throttle, i16 steer, LE).
}

/// A decoded command. Extend with new variants (e.g. `SetThrottle { throttle:
/// i16, steer: i16 }`) as new actuators arrive — dispatch generalizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    LedOff,
    LedOn,
    SetColor { r: u8, g: u8, b: u8 },
    SetBrightness(u8),
}

/// Why a byte slice failed to decode into a [`Command`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    UnknownId(u8),
    BadLength { id: u8, expected: usize, got: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Empty => write!(f, "empty command (need at least the id byte)"),
            DecodeError::UnknownId(id) => write!(f, "unknown command id 0x{id:02x}"),
            DecodeError::BadLength { id, expected, got } => write!(
                f,
                "command 0x{id:02x}: expected {expected} payload byte(s), got {got}"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

impl Command {
    /// The id byte this command encodes to.
    pub fn id(&self) -> u8 {
        match self {
            Command::LedOff => id::LED_OFF,
            Command::LedOn => id::LED_ON,
            Command::SetColor { .. } => id::SET_COLOR,
            Command::SetBrightness(_) => id::SET_BRIGHTNESS,
        }
    }

    /// Decode wire bytes (byte 0 = id, bytes 1.. = payload) into a `Command`.
    pub fn decode(bytes: &[u8]) -> Result<Command, DecodeError> {
        let (&id, payload) = bytes.split_first().ok_or(DecodeError::Empty)?;
        let need = |n: usize| -> Result<(), DecodeError> {
            if payload.len() == n {
                Ok(())
            } else {
                Err(DecodeError::BadLength { id, expected: n, got: payload.len() })
            }
        };
        match id {
            id::LED_OFF => {
                need(0)?;
                Ok(Command::LedOff)
            }
            id::LED_ON => {
                need(0)?;
                Ok(Command::LedOn)
            }
            id::SET_COLOR => {
                need(3)?;
                Ok(Command::SetColor { r: payload[0], g: payload[1], b: payload[2] })
            }
            id::SET_BRIGHTNESS => {
                need(1)?;
                Ok(Command::SetBrightness(payload[0]))
            }
            other => Err(DecodeError::UnknownId(other)),
        }
    }

    /// Encode to wire bytes — the exact inverse of [`Command::decode`].
    pub fn encode(&self) -> Vec<u8> {
        match *self {
            Command::LedOff => vec![id::LED_OFF],
            Command::LedOn => vec![id::LED_ON],
            Command::SetColor { r, g, b } => vec![id::SET_COLOR, r, g, b],
            Command::SetBrightness(v) => vec![id::SET_BRIGHTNESS, v],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all() {
        for cmd in [
            Command::LedOff,
            Command::LedOn,
            Command::SetColor { r: 255, g: 0, b: 0 },
            Command::SetColor { r: 0, g: 128, b: 255 },
            Command::SetBrightness(64),
        ] {
            assert_eq!(Command::decode(&cmd.encode()), Ok(cmd));
        }
    }

    #[test]
    fn set_color_red_bytes() {
        assert_eq!(Command::decode(&[0x02, 0xFF, 0x00, 0x00]), Ok(Command::SetColor { r: 255, g: 0, b: 0 }));
        assert_eq!(Command::SetColor { r: 255, g: 0, b: 0 }.encode(), vec![0x02, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn errors() {
        assert_eq!(Command::decode(&[]), Err(DecodeError::Empty));
        assert_eq!(Command::decode(&[0x99]), Err(DecodeError::UnknownId(0x99)));
        assert_eq!(Command::decode(&[0x02, 0xFF]), Err(DecodeError::BadLength { id: 0x02, expected: 3, got: 1 }));
        assert_eq!(Command::decode(&[0x00, 0x01]), Err(DecodeError::BadLength { id: 0x00, expected: 0, got: 1 }));
    }
}
