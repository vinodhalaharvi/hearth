//! Shared sensor/device drivers (I2C, SPI) for hearth ESP32-family nodes.
//!
//! Intentionally empty for now. When the first real sensor lands (e.g. a BME280
//! over I2C), its driver goes here so every board reuses it instead of each
//! firmware re-implementing bus access.
//!
//! Convention when you add one:
//!   - one submodule per device, e.g. `pub mod bme280;`
//!   - the constructor takes a shared I2C/SPI bus handle; no global state
//!   - return plain reading structs and let the firmware map them to
//!     MQTT topics/units (drivers stay transport-agnostic)
