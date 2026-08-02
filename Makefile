# hearth — convenience targets.
# NODE selects the firmware crate under firmware/ (default: esp32s3-sensor).
NODE      ?= esp32s3-sensor
NODE_DIR  := firmware/$(NODE)

# Topic root + broker for the `sub` watch target (defaults match .env.example).
SYSTEM    ?= hearth
MQTT_HOST ?= 192.168.68.139

# Recipes source .env (if present) so WIFI_/MQTT_/NODE_ID reach cargo as env vars.
# Fallbacks in the firmware mean an absent .env still compiles.
ENVLOAD = set -a; [ -f .env ] && . ./.env; set +a

.PHONY: build run monitor sub env help

## build: compile the selected node (override with NODE=...)
build:
	$(ENVLOAD); cd $(NODE_DIR) && cargo build --release

## run: build + flash + serial monitor (the configured cargo runner does all three)
run:
	$(ENVLOAD); cd $(NODE_DIR) && cargo run --release

## monitor: open the serial monitor only
monitor:
	espflash monitor

## sub: watch this system's MQTT topics (needs mosquitto-clients)
sub:
	mosquitto_sub -h $(MQTT_HOST) -t '$(SYSTEM)/#' -v

## env: show the config the build will use (does not print secrets' values)
env:
	$(ENVLOAD); echo "NODE      = $(NODE)"; echo "SSID      = $${WIFI_SSID:-<unset>}"; echo "MQTT_HOST = $${MQTT_HOST:-<unset>}"; echo "MQTT_PORT = $${MQTT_PORT:-1883}"; echo "NODE_ID   = $${NODE_ID:-<unset>}"

## help: list targets
help:
	grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## //'
