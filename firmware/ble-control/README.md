# ble-control

A BLE GATT peripheral that controls the ESP32-S3 DevKitC's **onboard WS2812 RGB
LED** via structured commands. This is a **separate subsystem** from hearth's
WiFi/MQTT telemetry: BLE direct control, no WiFi, no broker, no credentials.

The reusable core lives in `hearth-shared`:

- `command` — the byte wire-format below (pure, host-tested).
- `ble` — the NimBLE peripheral (one service, one writable command
  characteristic, one read/notify status characteristic).

This crate is thin: it owns the LED and maps a decoded `Command` to pixels. A
future actuator (motor, servo) reuses `ble` + `command` unchanged and adds a new
`Command` variant + handler.

- **Board:** ESP32-S3-DevKitC-1 (Xtensa). Not the XIAO.
- **LED GPIO:** **48** (onboard addressable WS2812). Set in `src/main.rs`
  (`LED_GPIO` / the `gpio48` object). Verify against your board revision.
- **BLE stack:** NimBLE via `esp32-nimble` 0.12 (matches `esp-idf-svc` 0.52 in
  this workspace). Bluedroid is disabled in `sdkconfig.defaults`.

## Command wire format (the real interface)

Bytes written to the **command characteristic**. Explicit byte layout — not Rust
memory layout:

```
byte 0      = command id
bytes 1..N  = payload (depends on id)
```

| id (hex) | command       | payload         | example bytes   | effect              |
|----------|---------------|-----------------|-----------------|---------------------|
| `00`     | LedOff        | (none)          | `00`            | LED off             |
| `01`     | LedOn         | (none)          | `01`            | LED on, last color  |
| `02`     | SetColor      | R, G, B (3)     | `02 FF 00 00`   | red                 |
| `03`     | SetBrightness | level 0–255 (1) | `03 80`         | ~50% brightness     |

Ready-to-write examples:

- **Red:**   `02 FF 00 00`
- **Green:** `02 00 FF 00`
- **Blue:**  `02 00 00 FF`
- **White:** `02 FF FF FF`
- **Off:**   `00`
- **On:**    `01`
- **50% brightness:** `03 80`

Malformed writes (unknown id, wrong length) are logged and ignored.

The **status characteristic** (read/notify) reports current state as
`[on, r, g, b, brightness]` (5 bytes) after each command.

## BLE UUIDs (128-bit)

| role                 | UUID                                   | properties      |
|----------------------|----------------------------------------|-----------------|
| service              | `9f1e0000-1a2b-4c3d-8e9f-a0b1c2d3e4f5` | —               |
| command characteristic | `9f1e0001-1a2b-4c3d-8e9f-a0b1c2d3e4f5` | Write / Write-No-Response |
| status characteristic  | `9f1e0002-1a2b-4c3d-8e9f-a0b1c2d3e4f5` | Read / Notify   |

## Flash + monitor

```bash
cd ~/embedded_projects/hearth
make run NODE=ble-control        # or: cd firmware/ble-control && cargo run
```

The DevKitC-1 has both a UART-bridge port and a native-USB port. Same monitor
quirk as the XIAO on the native-USB port: `CTRL+R` resets the chip, `CTRL+C`
exits `espflash monitor`. Serial logs print on connect/disconnect and on every
received command.

## Test with nRF Connect (the "done" definition)

1. Flash (above). Serial should show `advertising as 'hearth-ble-01'`.
2. Open **nRF Connect** (iOS/Android) → **Scan** → connect to **`hearth-ble-01`**.
3. Expand the service `9f1e0000-…` and find the **command** characteristic
   `9f1e0001-…`. Tap its **write** (up-arrow) button; set value type to **Bytes
   (hex)**.
4. Write these and watch the LED:
   - `02 FF 00 00` → **red**
   - `02 00 FF 00` → **green**
   - `02 00 00 FF` → **blue**
   - `00` → **off**
   - `03 40` then `02 FF 00 00` → dim red (25% brightness)
5. (Optional) Enable notifications on the **status** characteristic `9f1e0002-…`
   to read back `[on, r, g, b, brightness]`.

Each write also prints to the serial monitor, e.g. `BLE command <- [02, ff, 00, 00]`
then `command: SetColor { r: 255, g: 0, b: 0 }`.
