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
}
