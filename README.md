# hearth

A multi-device embedded system. Several microcontroller **nodes** sense and
publish to MQTT; **consumers** (a Go controller, Home Assistant, loggers,
dashboards) subscribe independently. Nodes know nothing about who consumes their
data — **MQTT is the decoupling seam.**

This repo is the evolution of an earlier ESP8266/Arduino proof-of-concept. The
firmware here is **Rust on the ESP32 family** (`esp-idf-svc`, std route). Today
it has one real node — an ESP32-S3 publishing onboard telemetry — plus the
scaffolding to add more without restructuring.

## The MQTT contract (the system's real API)

Because MQTT is the seam, the **topic hierarchy and payload shape are the
interface** — more important than any single consumer. Every node and every
consumer depends on this. Keep it stable and consistent.

### Topics

```
hearth/<node-id>/<metric>     # a reading, e.g. hearth/esp32s3-01/rssi
hearth/<node-id>/status       # retained availability: "online" / "offline"
```

- `hearth` is the system name and the topic root.
- `<node-id>` is unique and **stable per physical board** (e.g. `esp32s3-01`).
- `<metric>` is the reading name (`rssi`, `heap`, later `temperature`, …).

### Payload

Small, self-describing JSON, consistent across nodes so consumers parse uniformly:

```json
{ "device": "esp32s3-01", "uptime_ms": 12345, "value": -54, "unit": "dBm" }
```

`unit` is omitted when not meaningful. `device` echoes the node id; `uptime_ms`
timestamps the sample relative to boot.

### Availability (LWT)

Each node registers a **Last Will and Testament** on its `status` topic:
`online` (retained) is published on connect; the broker publishes `offline`
(retained) automatically if the node drops. This MQTT-native behavior is exactly
why MQTT beats raw HTTP here — consumers learn about dead nodes for free.

### QoS

Readings publish at **QoS 1** (at-least-once) by default, so a briefly
disconnected consumer doesn't miss samples. Revisit per-metric if a reason arises.

Broker connection details (host, port, credentials) are **configuration**, never
hardcoded — see `.env.example`.

## Layout

```
hearth/
├── README.md              # this file — vision + the MQTT contract
├── .env.example           # config template (WiFi + MQTT); real .env gitignored
├── Makefile               # per-node build/run + a topic-watch helper
├── docs/                  # developer guides
├── shared/                # shared Rust crate for the ESP32 family
│   └── src/               #   mqtt (transport + contract), drivers, manifest
├── consumers/             # signpost: consumers live in their own repos
└── firmware/
    ├── README.md          # how to add a node (the convention)
    └── esp32s3-sensor/    # the one real node today
```

## Quick start (ESP32-S3 node)

Install the Espressif Rust toolchain:

```bash
cargo install espup ldproxy espflash
espup install                 # installs the "esp" (Xtensa) toolchain
. $HOME/export-esp.sh          # env for this shell (add to your profile)
```

Configure and flash:

```bash
cp .env.example .env           # fill WIFI_SSID/PASS; MQTT_HOST is the local broker
make run                       # build + flash + serial monitor
```

Watch the readings land on the broker (from anywhere on the LAN):

```bash
mosquitto_sub -h 192.168.68.139 -t 'hearth/#' -v    # or: make sub
```

You'll see `hearth/esp32s3-01/status online`, then `hearth/esp32s3-01/rssi …`
and `…/heap …` every few seconds. **That — a real message on the topic, not a
clean compile — is the milestone.**

## Adding a node

See [`firmware/README.md`](firmware/README.md). Short version: copy a firmware
folder, give it a unique `NODE_ID`, reuse `hearth-shared`, and add one line to
the workspace `members` list.
