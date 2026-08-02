//! MQTT transport and the hearth wire contract.
//!
//! **This module defines the system's public API.** The topic layout and the
//! payload shape here are what every node publishes and every consumer parses —
//! that contract matters more than any single consumer. See the root `README.md`.
//!
//! Contract in brief:
//!   - topics:  `hearth/<node-id>/<metric>`  and  `hearth/<node-id>/status`
//!   - payload: small self-describing JSON, e.g.
//!              `{"device":"esp32s3-01","uptime_ms":1234,"value":-54,"unit":"dBm"}`
//!   - status:  retained; `online` on connect, LWT `offline` on disconnect
//!   - QoS:     1 (at-least-once) for readings, so consumers don't miss samples

use anyhow::Result;
use esp_idf_svc::mqtt::client::{
    EspMqttClient, LwtConfiguration, MqttClientConfiguration, QoS,
};

/// Default QoS for readings. At-least-once: a briefly-disconnected consumer
/// still receives the sample (at the cost of possible duplicates).
pub const READING_QOS: QoS = QoS::AtLeastOnce;

/// Broker connection settings, sourced from the environment / `.env`
/// (never hardcoded). `url` is assembled from host + port.
pub struct MqttSettings {
    pub url: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
}

impl MqttSettings {
    /// Build settings from discrete parts (as injected from `.env`). The
    /// client id is derived from the node id so it is unique per board.
    pub fn from_parts(host: &str, port: &str, username: &str, password: &str, node_id: &str) -> Self {
        Self {
            url: format!("mqtt://{host}:{port}"),
            client_id: format!("hearth-{node_id}"),
            username: username.to_string(),
            password: password.to_string(),
        }
    }
}

/// The last three bytes of the factory MAC as hex — a stable, chip-unique
/// suffix (e.g. `"3c9a1f"`). Read straight from eFuse; needs no WiFi/NVS init.
pub fn chip_id_suffix() -> String {
    let mut mac = [0u8; 6];
    unsafe {
        esp_idf_svc::sys::esp_efuse_mac_get_default(mac.as_mut_ptr());
    }
    format!("{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5])
}

/// Resolve this board's node id. A non-empty explicit id (from `.env`/env) wins,
/// so a board can have a meaningful, stable name; otherwise fall back to
/// `{prefix}-{chip_id_suffix}`, unique per physical chip. This is what stops a
/// freshly-flashed second board of the same type from colliding on one id.
pub fn resolve_node_id(explicit: Option<&str>, prefix: &str) -> String {
    match explicit {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => format!("{prefix}-{}", chip_id_suffix()),
    }
}

/// The per-node topic set, rooted at the system name (the MQTT topic root).
pub struct Topics {
    base: String,
}

impl Topics {
    /// `system` is the topic root ("hearth"); `node_id` is the stable board id.
    pub fn new(system: &str, node_id: &str) -> Self {
        Self { base: format!("{system}/{node_id}") }
    }
    /// Topic for a named metric: `metric("rssi") -> "hearth/esp32s3-01/rssi"`.
    pub fn metric(&self, metric: &str) -> String {
        format!("{}/{}", self.base, metric)
    }
    /// The retained availability topic: "hearth/esp32s3-01/status".
    pub fn status(&self) -> String {
        format!("{}/status", self.base)
    }
}

/// A small, self-describing reading payload as JSON. `value` is anything
/// `Display` (int/float); an empty `unit` is omitted. Hand-rolled to avoid a
/// serde dependency while the payload is this simple — swap to serde if it grows.
pub fn reading_json(device: &str, uptime_ms: u64, value: impl core::fmt::Display, unit: &str) -> String {
    if unit.is_empty() {
        format!(r#"{{"device":"{device}","uptime_ms":{uptime_ms},"value":{value}}}"#)
    } else {
        format!(r#"{{"device":"{device}","uptime_ms":{uptime_ms},"value":{value},"unit":"{unit}"}}"#)
    }
}

/// Home Assistant MQTT-discovery prefix (HA's default). Retained config topics
/// under this prefix make HA auto-create a device + entities per node.
pub const HA_DISCOVERY_PREFIX: &str = "homeassistant";

/// Board identity for the Home Assistant device card. Model + manufacturer are
/// board-specific, so each firmware supplies them; everything else about
/// discovery (topics, entities, availability) lives here as part of the
/// contract, so every node advertises itself the same way.
pub struct DeviceInfo<'a> {
    pub manufacturer: &'a str,
    pub model: &'a str,
    pub sw_version: &'a str,
}

/// HA metadata for one published metric. Adding a row here (and publishing that
/// metric in the loop) is all it takes for a new reading to appear in HA.
struct MetricMeta {
    key: &'static str,
    name: &'static str,
    unit: &'static str,
    device_class: &'static str,
}

/// The metrics every node currently publishes (kept in step with the loop).
const HA_METRICS: &[MetricMeta] = &[
    MetricMeta { key: "rssi", name: "RSSI",      unit: "dBm", device_class: "signal_strength" },
    MetricMeta { key: "heap", name: "Free heap", unit: "B",   device_class: "data_size" },
];

/// A connected node: owns the MQTT client and knows its own identity/topics.
///
/// Construction sets a Last-Will on the status topic so the broker publishes a
/// retained `offline` if this node drops — the MQTT-native feature that makes
/// this a better decoupling seam than raw HTTP.
pub struct Node {
    client: EspMqttClient<'static>,
    device: String,
    topics: Topics,
}

impl Node {
    /// Connect to the broker with an LWT on `<system>/<node_id>/status`.
    ///
    /// Spawns a background thread to service the MQTT event loop (esp-idf-svc
    /// requires someone to poll the connection); a publisher just drains it.
    pub fn connect(settings: &MqttSettings, system: &str, node_id: &str) -> Result<Self> {
        let topics = Topics::new(system, node_id);
        let status_topic = topics.status();

        let conf = MqttClientConfiguration {
            client_id: Some(&settings.client_id),
            username: (!settings.username.is_empty()).then_some(settings.username.as_str()),
            password: (!settings.password.is_empty()).then_some(settings.password.as_str()),
            lwt: Some(LwtConfiguration {
                topic: &status_topic,
                payload: b"offline",
                qos: QoS::AtLeastOnce,
                retain: true,
            }),
            ..Default::default()
        };

        let (client, mut connection) = EspMqttClient::new(&settings.url, &conf)?;

        std::thread::Builder::new()
            .stack_size(6144)
            .spawn(move || {
                // Drain events until the connection closes.
                while connection.next().is_ok() {}
                log::warn!("mqtt connection loop ended");
            })?;

        Ok(Self { client, device: node_id.to_string(), topics })
    }

    /// Publish retained `online` to the status topic (call once after connect).
    pub fn publish_status_online(&mut self) -> Result<()> {
        self.client
            .publish(&self.topics.status(), QoS::AtLeastOnce, true, b"online")?;
        Ok(())
    }

    /// Publish one metric reading as JSON to `hearth/<node>/<metric>`.
    pub fn publish_metric(
        &mut self,
        metric: &str,
        value: impl core::fmt::Display,
        unit: &str,
        uptime_ms: u64,
    ) -> Result<()> {
        let topic = self.topics.metric(metric);
        let payload = reading_json(&self.device, uptime_ms, value, unit);
        self.client
            .publish(&topic, READING_QOS, false, payload.as_bytes())?;
        Ok(())
    }

    /// Announce this node to Home Assistant via retained MQTT discovery: one HA
    /// device (grouping every entity) with a sensor per metric plus an
    /// online/offline connectivity entity. Sensor availability is tied to the
    /// LWT status topic, so entities show "unavailable" the instant the node
    /// drops. Call once, right after connecting.
    pub fn announce_ha_discovery(&mut self, dev: &DeviceInfo) -> Result<()> {
        let device_uid = format!("hearth-{}", self.device);
        let status_topic = self.topics.status();

        // Shared "device" block — puts every entity on one HA device card.
        let device = format!(
            r#""device":{{"identifiers":["{uid}"],"name":"hearth {node}","manufacturer":"{mfr}","model":"{model}","sw_version":"{sw}"}}"#,
            uid = device_uid,
            node = self.device,
            mfr = dev.manufacturer,
            model = dev.model,
            sw = dev.sw_version,
        );

        // A sensor per metric: value pulled from the JSON payload, availability
        // from the retained status topic.
        for m in HA_METRICS {
            let config_topic =
                format!("{HA_DISCOVERY_PREFIX}/sensor/{device_uid}/{}/config", m.key);
            let payload = format!(
                r#"{{"name":"{name}","unique_id":"{uid}-{key}","state_topic":"{state}","value_template":"{tmpl}","unit_of_measurement":"{unit}","device_class":"{dc}","state_class":"measurement","entity_category":"diagnostic","availability_topic":"{status}","payload_available":"online","payload_not_available":"offline",{device}}}"#,
                name = m.name,
                uid = device_uid,
                key = m.key,
                state = self.topics.metric(m.key),
                tmpl = "{{ value_json.value }}",
                unit = m.unit,
                dc = m.device_class,
                status = status_topic,
                device = device,
            );
            self.client
                .publish(&config_topic, QoS::AtLeastOnce, true, payload.as_bytes())?;
        }

        // Connectivity entity reads the retained status topic directly
        // (online -> on, offline -> off). Deliberately no availability here, or
        // it would hide its own "disconnected" state when the LWT fires.
        let status_config_topic =
            format!("{HA_DISCOVERY_PREFIX}/binary_sensor/{device_uid}/status/config");
        let status_payload = format!(
            r#"{{"name":"Status","unique_id":"{uid}-status","state_topic":"{status}","payload_on":"online","payload_off":"offline","device_class":"connectivity","entity_category":"diagnostic",{device}}}"#,
            uid = device_uid,
            status = status_topic,
            device = device,
        );
        self.client
            .publish(&status_config_topic, QoS::AtLeastOnce, true, status_payload.as_bytes())?;

        log::info!(
            "announced Home Assistant discovery: {} sensors + status",
            HA_METRICS.len()
        );
        Ok(())
    }
}
