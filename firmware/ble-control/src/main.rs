//! hearth · ble-control
//!
//! A BLE GATT peripheral that controls the onboard WS2812 RGB LED via structured
//! commands written to a single "command" characteristic. This is a **separate
//! subsystem** from hearth's WiFi/MQTT telemetry — no WiFi, no broker, no creds.
//!
//! The reusable parts live in `hearth-shared`:
//!   - `command` — the byte wire-format (see that module / this crate's README)
//!   - `ble`     — the NimBLE peripheral (service + writable command char)
//! This firmware is thin: it owns the LED and maps decoded `Command`s to pixels.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::{error, info, warn};
use smart_leds::{SmartLedsWrite, RGB8};
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use hearth_shared::ble;
use hearth_shared::command::Command;

/// BLE device name shown in nRF Connect (and in the advertisement).
const DEVICE_NAME: &str = "hearth-ble-01";

/// Onboard addressable RGB LED (WS2812) data pin.
/// ESP32-S3-DevKitC-1 = GPIO48. VERIFY on your board — revisions/clones differ.
/// (The GPIO object taken below must match this number.)
const LED_GPIO: u8 = 48;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    // WS2812 on the onboard RGB LED, driven over the RMT peripheral.
    let ws2812 = Ws2812Esp32Rmt::new(peripherals.rmt.channel0, peripherals.pins.gpio48)?;
    let led = Arc::new(Mutex::new(LedController::new(ws2812)));
    led.lock().unwrap().render()?; // start dark

    info!("ble-control up — LED on GPIO{LED_GPIO}, advertising as '{DEVICE_NAME}'");

    // Wire the shared BLE peripheral to the concrete LED handler. Runs in the
    // NimBLE host task; decoding + LED write are fast, so we handle inline.
    let led_for_ble = led.clone();
    ble::start(DEVICE_NAME, move |bytes, status| {
        match Command::decode(bytes) {
            Ok(cmd) => {
                info!("command: {cmd:?}");
                let mut led = led_for_ble.lock().unwrap();
                if let Err(e) = led.apply(cmd) {
                    error!("LED apply failed: {e:?}");
                }
                status.set(&led.state_bytes()); // report state back to clients
            }
            Err(e) => warn!("ignoring malformed command {bytes:02x?}: {e}"),
        }
    })?;

    // BLE runs in its own task; keep the main thread alive.
    loop {
        std::thread::sleep(Duration::from_secs(5));
    }
}

/// Owns the WS2812 driver and current LED state; maps decoded commands to the
/// single onboard pixel. (Future actuators get their own controller like this;
/// the BLE + command layers don't change.)
struct LedController {
    ws2812: Ws2812Esp32Rmt<'static>,
    on: bool,
    color: RGB8,
    brightness: u8,
}

impl LedController {
    fn new(ws2812: Ws2812Esp32Rmt<'static>) -> Self {
        Self { ws2812, on: false, color: RGB8::new(255, 255, 255), brightness: 255 }
    }

    fn apply(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::LedOff => self.on = false,
            Command::LedOn => self.on = true,
            Command::SetColor { r, g, b } => {
                self.color = RGB8::new(r, g, b);
                self.on = true;
            }
            Command::SetBrightness(v) => self.brightness = v,
        }
        self.render()
    }

    fn render(&mut self) -> Result<()> {
        let px = if self.on {
            scale(self.color, self.brightness)
        } else {
            RGB8::new(0, 0, 0)
        };
        self.ws2812.write([px])?;
        Ok(())
    }

    /// Status bytes reported over the BLE status characteristic:
    /// `[on(0/1), r, g, b, brightness]`.
    fn state_bytes(&self) -> [u8; 5] {
        [self.on as u8, self.color.r, self.color.g, self.color.b, self.brightness]
    }
}

/// Scale an RGB color by a 0..=255 brightness.
fn scale(c: RGB8, b: u8) -> RGB8 {
    let s = |v: u8| ((v as u16 * b as u16) / 255) as u8;
    RGB8::new(s(c.r), s(c.g), s(c.b))
}
