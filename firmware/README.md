# firmware

Each node is its own Cargo crate here and a member of the workspace at the repo
root. Nodes share code via the `hearth-shared` crate (a path dependency) — most
importantly its `mqtt` module, which owns the topic/payload/LWT contract so
every node speaks the same wire format.

## Targets

| Crate | Board | Node id | Board-specifics |
|-------|-------|---------|-----------------|
| `esp32s3-sensor` | ESP32-S3-DevKitC | `esp32s3-<mac>` | LED GPIO48; UART-bridge serial console |
| `xiao-sense` | Seeed XIAO ESP32-S3 Sense | `xiao-<mac>` | LED GPIO21 (active-low); native USB Serial/JTAG console; 8 MB OPI PSRAM |

Both crates run the **same** telemetry code and publish through
`hearth_shared::mqtt` — the only differences are the crate name, the `NODE_PREFIX`
(board family; the node id is chip-derived), and board-specific
pin/USB/PSRAM config. First light for any node is: WiFi connect, MQTT connect
with LWT, and `rssi` + `heap` published under `hearth/<node-id>/…`.

## How to add a node

1. **Copy an existing node** as a starting point:
   ```bash
   cp -r firmware/esp32s3-sensor firmware/<new-node>
   ```
2. **Rename the crate** in `firmware/<new-node>/Cargo.toml` (the `name` and the
   `[[bin]]` name).
3. **Retarget if it's a different chip.** Adjust `.cargo/config.toml`
   (`target = …`), `rust-toolchain.toml` (Xtensa `esp` vs. RISC-V stable), and
   `sdkconfig.defaults` (PSRAM, etc.) for the new board. The `esp32s3-sensor`
   files are the reference for an S3.
4. **Register it** in the root `Cargo.toml`:
   ```toml
   [workspace]
   members = [
       "shared",
       "firmware/esp32s3-sensor",
       "firmware/<new-node>",   # <- one line
   ]
   ```
5. **Reuse shared code** (already wired if you copied):
   ```toml
   hearth-shared = { path = "../../shared" }
   ```
   Use `hearth_shared::mqtt` for transport + topics so the node honors the
   contract automatically. Put any new sensor driver in `shared/src/drivers.rs`
   so other boards get it too.
6. **Identity is automatic.** With no `NODE_ID` set, each board self-names as
   `<NODE_PREFIX>-<chip-mac-suffix>` (unique per physical chip), so a second
   board of the same type never collides. Set `NODE_PREFIX` per crate (board
   family) and export `NODE_ID` only to give one board a stable friendly name.

That's the whole ceremony: a node is a folder that reuses `shared/` and adds one
line to the workspace. No board folders are pre-created — add them when the board
is real.

## Building a node

esp-idf config (target, toolchain) is per-node, so build **from inside the
node's directory** — or use the root `Makefile` targets, which `cd` for you:

```bash
cd firmware/esp32s3-sensor
cargo run            # build + flash + monitor (via the configured runner)
```

Or drive it from the repo root with `NODE=`:

```bash
make run NODE=xiao-sense     # build + flash + monitor the XIAO (omit NODE for esp32s3-sensor)
make sub                     # watch hearth/# — both nodes at once
```
