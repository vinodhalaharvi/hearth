# consumers

Nothing lives here yet — and that's the point.

`hearth` nodes publish to MQTT and know nothing about who consumes their data.
Consumers — a Go controller, Home Assistant, loggers, dashboards — subscribe to
the broker independently and live in **their own repos**, or later as
subdirectories here if it's convenient to co-locate one.

The only contract a consumer needs is the MQTT topic/payload spec in the root
[`README.md`](../README.md). Anything that speaks to the broker and honors that
contract is a valid consumer. This folder is a signpost, not a home for node code.
