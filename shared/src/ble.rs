//! Reusable BLE GATT peripheral for structured byte commands (NimBLE).
//!
//! One service with a **writable "command" characteristic** and a
//! **read/notify "status" characteristic**. Bytes written to the command
//! characteristic are handed verbatim to a dispatch closure — this module knows
//! nothing about the command *format* (see [`crate::command`]), so it is
//! reusable for any byte protocol and any future actuator (motor, servo, …).
//!
//! Requires the `ble` cargo feature (pulls `esp32-nimble`). NimBLE must be
//! enabled in the firmware's `sdkconfig.defaults` (see the ble-control crate).
//!
//! UUIDs (128-bit, custom) — documented so a phone/app author can find them:
//!   service        9f1e0000-1a2b-4c3d-8e9f-a0b1c2d3e4f5
//!   command  (W)   9f1e0001-1a2b-4c3d-8e9f-a0b1c2d3e4f5
//!   status  (R/N)  9f1e0002-1a2b-4c3d-8e9f-a0b1c2d3e4f5

use std::sync::Arc;

use anyhow::Result;
use esp32_nimble::{
    utilities::mutex::Mutex, uuid128, BLEAdvertisementData, BLECharacteristic, BLEDevice,
    NimbleProperties,
};

/// Custom service/characteristic UUID strings, exposed for clients/tests and
/// documentation. Keep these in sync with the `uuid128!` literals used below.
pub const SERVICE_UUID: &str = "9f1e0000-1a2b-4c3d-8e9f-a0b1c2d3e4f5";
pub const CMD_CHAR_UUID: &str = "9f1e0001-1a2b-4c3d-8e9f-a0b1c2d3e4f5";
pub const STATUS_CHAR_UUID: &str = "9f1e0002-1a2b-4c3d-8e9f-a0b1c2d3e4f5";

/// Handle to the read/notify status characteristic — push state back to clients.
/// Cheap to clone (an `Arc`); safe to move into the command handler.
#[derive(Clone)]
pub struct StatusHandle(Arc<Mutex<BLECharacteristic>>);

impl StatusHandle {
    /// Set the status value and notify any subscribed clients.
    pub fn set(&self, bytes: &[u8]) {
        self.0.lock().set_value(bytes).notify();
    }
}

/// Start the BLE command peripheral and begin advertising as `device_name`.
///
/// `on_command` runs in the NimBLE host task on every write to the command
/// characteristic, receiving the raw bytes and a [`StatusHandle`] to report
/// state back. Setting an LED is fast, so handling inline is fine; a *slow*
/// actuator should hand the bytes off to its own task rather than block here.
///
/// Returns a [`StatusHandle`] the caller may also keep to push status from
/// elsewhere (e.g. a main-loop heartbeat).
pub fn start<F>(device_name: &str, mut on_command: F) -> Result<StatusHandle>
where
    F: FnMut(&[u8], &StatusHandle) + Send + Sync + 'static,
{
    let device = BLEDevice::take();
    let server = device.get_server();

    server.on_connect(|_server, desc| {
        log::info!("BLE client connected: {desc:?}");
    });
    server.on_disconnect(|_desc, _reason| {
        log::info!("BLE client disconnected");
    });

    // One literal, reused (BleUuid is Copy) for the service and the advert.
    let service_uuid = uuid128!("9f1e0000-1a2b-4c3d-8e9f-a0b1c2d3e4f5");
    let service = server.create_service(service_uuid);

    // Read/notify status characteristic.
    let status_char = service.lock().create_characteristic(
        uuid128!("9f1e0002-1a2b-4c3d-8e9f-a0b1c2d3e4f5"),
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );
    let status = StatusHandle(status_char);

    // Writable command characteristic — the actual control surface.
    let cmd_char = service.lock().create_characteristic(
        uuid128!("9f1e0001-1a2b-4c3d-8e9f-a0b1c2d3e4f5"),
        NimbleProperties::WRITE | NimbleProperties::WRITE_NO_RSP,
    );
    let status_for_cb = status.clone();
    cmd_char.lock().on_write(move |args| {
        let data = args.recv_data();
        log::info!("BLE command <- {data:02x?}");
        on_command(data, &status_for_cb);
    });

    // Advertise so nRF Connect / any client can find and connect.
    let advertising = device.get_advertising();
    advertising
        .lock()
        .set_data(
            BLEAdvertisementData::new()
                .name(device_name)
                .add_service_uuid(service_uuid),
        )
        .map_err(|e| anyhow::anyhow!("BLE set_data failed: {e:?}"))?;
    advertising
        .lock()
        .start()
        .map_err(|e| anyhow::anyhow!("BLE advertising start failed: {e:?}"))?;

    log::info!("BLE advertising as '{device_name}' (service {SERVICE_UUID})");
    Ok(status)
}
