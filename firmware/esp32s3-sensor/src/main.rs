//! hearth · esp32s3-sensor
//!
//! Connects to WiFi, connects to the MQTT broker with a Last-Will, blinks the
//! onboard LED for liveness, and every few seconds publishes real onboard
//! readings (WiFi RSSI, free heap) to `hearth/<node>/<metric>`.
//!
//! Config (WiFi + broker + node id) is injected at build time via `option_env!`,
//! with placeholder fallbacks so this still compiles without a `.env`. The
//! Makefile sources `.env` into the environment before `cargo run`.

use std::time::{Duration, Instant};

use anyhow::Result;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::PinDriver;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use log::info;

use hearth_shared::mqtt::{DeviceInfo, MqttSettings, Node};

// --- config: injected from .env at build time, with safe fallbacks ---
const SYSTEM: &str = "hearth";
const WIFI_SSID: &str = match option_env!("WIFI_SSID") { Some(v) => v, None => "changeme-ssid" };
const WIFI_PASS: &str = match option_env!("WIFI_PASS") { Some(v) => v, None => "changeme-pass" };
const MQTT_HOST: &str = match option_env!("MQTT_HOST") { Some(v) => v, None => "192.168.68.139" };
const MQTT_PORT: &str = match option_env!("MQTT_PORT") { Some(v) => v, None => "1883" };
const MQTT_USER: &str = match option_env!("MQTT_USER") { Some(v) => v, None => "" };
const MQTT_PASS: &str = match option_env!("MQTT_PASS") { Some(v) => v, None => "" };
const NODE_ID: &str = match option_env!("NODE_ID") { Some(v) => v, None => "esp32s3-01" };

// Board identity for the Home Assistant device card (board-specific).
const BOARD_MFR: &str = "Espressif";
const BOARD_MODEL: &str = "ESP32-S3-DevKitC";

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Onboard LED. Many ESP32-S3 devkits expose a simple LED on GPIO48 (some use
    // an addressable RGB there instead — adjust for your board if it doesn't blink).
    let mut led = PinDriver::output(peripherals.pins.gpio48)?;

    // --- WiFi ---
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;
    connect_wifi(&mut wifi)?;
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    info!("wifi up, ip: {}", ip.ip);

    // --- MQTT ---
    let settings = MqttSettings::from_parts(MQTT_HOST, MQTT_PORT, MQTT_USER, MQTT_PASS, NODE_ID);
    let mut node = Node::connect(&settings, SYSTEM, NODE_ID)?;
    node.publish_status_online()?;
    node.announce_ha_discovery(&DeviceInfo {
        manufacturer: BOARD_MFR,
        model: BOARD_MODEL,
        sw_version: env!("CARGO_PKG_VERSION"),
    })?;
    info!("mqtt connected as '{NODE_ID}'; publishing under {SYSTEM}/{NODE_ID}/");

    // --- publish loop ---
    let start = Instant::now();
    loop {
        // liveness blink
        led.set_high()?;
        std::thread::sleep(Duration::from_millis(50));
        led.set_low()?;

        let uptime_ms = start.elapsed().as_millis() as u64;
        let rssi = wifi_rssi();
        let heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };

        node.publish_metric("rssi", rssi, "dBm", uptime_ms)?;
        node.publish_metric("heap", heap, "B", uptime_ms)?;
        info!("published rssi={rssi} dBm, heap={heap} B, uptime={uptime_ms} ms");

        std::thread::sleep(Duration::from_secs(5));
    }
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> Result<()> {
    let config = Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID
            .try_into()
            .map_err(|_| anyhow::anyhow!("WIFI_SSID too long"))?,
        password: WIFI_PASS
            .try_into()
            .map_err(|_| anyhow::anyhow!("WIFI_PASS too long"))?,
        ..Default::default()
    });
    wifi.set_configuration(&config)?;
    wifi.start()?;
    info!("connecting to wifi '{WIFI_SSID}'...");
    wifi.connect()?;
    wifi.wait_netif_up()?;
    Ok(())
}

/// Current station RSSI in dBm — a real onboard measurement.
fn wifi_rssi() -> i32 {
    let mut rssi: i32 = 0;
    unsafe {
        esp_idf_svc::sys::esp_wifi_sta_get_rssi(&mut rssi);
    }
    rssi
}
