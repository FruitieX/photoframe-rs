# Photo Frame Receiver

## Build Requirements

- Board: Seeed XIAO ESP32-C6
- Espressif Arduino core: 3.3.10

Copy `config.example.h` to `config.h` and set the local hostname, OTA password, Wi-Fi SSID, Wi-Fi password, and panel dimensions. `config.h` is intentionally ignored by Git.

## Upload API

`POST /upload` accepts exactly `EPD_WIDTH * EPD_HEIGHT / 2` bytes as either a `multipart/form-data` file upload or an `application/octet-stream` request body. Other sizes and display failures return an HTTP error.

`GET /health` reports heap, minimum heap, Wi-Fi RSSI, time since the last successful upload, and the latest reset reason.
