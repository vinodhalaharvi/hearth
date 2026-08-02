# firmware

Each node is its own Cargo crate here and a member of the workspace at the repo
root. Nodes share code via the `hearth-shared` crate (a path dependency) — most
importantly its `mqtt` module, which owns the topic/payload/LWT contract so
every node speaks the same wire format.

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
6. **Give it a unique `NODE_ID`** (via `.env`) so its topics don't collide.

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
