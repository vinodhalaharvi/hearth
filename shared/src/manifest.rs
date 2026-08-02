//! Node manifest + discovery/codegen tooling (stub).
//!
//! Intent: a node declares the sensors/metrics it exposes in one place (a
//! "manifest"), and tooling turns that into (a) the MQTT topics it publishes
//! and (b) optional Home Assistant discovery configs — so adding a metric is a
//! data change, not edits scattered across the firmware.
//!
//! Not built yet. Documented here so the seam exists; implement it once a node
//! has enough metrics to justify the machinery.
