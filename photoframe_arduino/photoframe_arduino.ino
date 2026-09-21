// ==== Configuration ====
// Copy config.example.h to config.h and fill in the local values.
#include "config.h"

#include <ArduinoOTA.h>
#include <ESPmDNS.h>
#include <SPI.h>
#include <WebServer.h>
#include <WiFi.h>
#include <esp_system.h>
#include <esp_task_wdt.h>

#include "GDEP040E01.h"

const char *ssid = WIFI_SSID;
const char *password = WIFI_PASSWORD;

WebServer server(80);

static const unsigned long TASK_WDT_TIMEOUT_MS = 120000;
static const unsigned long WIFI_CHECK_INTERVAL_MS = 10000;
static const unsigned long WIFI_RECONNECT_INTERVAL_MS = 30000;
static const unsigned long WIFI_REBOOT_TIMEOUT_MS = 5UL * 60 * 1000;
static const unsigned long SERVICE_REBOOT_TIMEOUT_MS = 3UL * 60 * 60 * 1000;
static const unsigned long UPLOAD_TIMEOUT_MS = 5UL * 60 * 1000;

enum UploadResult {
  UPLOAD_NONE,
  UPLOAD_IN_PROGRESS,
  UPLOAD_SUCCESS,
  UPLOAD_SIZE_MISMATCH,
  UPLOAD_TOO_LARGE,
  UPLOAD_PANEL_FAILURE,
  UPLOAD_ABORTED,
};

static size_t g_bytesReceived = 0;
static UploadResult g_uploadResult = UPLOAD_NONE;
static bool g_uploadActive = false;
static bool g_uploadComplete = false;
static bool g_wifiWasConnected = false;
static bool g_networkServicesStarted = false;
static bool g_restartPending = false;
static unsigned long g_restartAt = 0;
static unsigned long g_wifiLostSince = 0;
static unsigned long g_lastWifiCheck = 0;
static unsigned long g_lastWifiReconnect = 0;
static unsigned long g_lastSuccessfulUpload = 0;
static unsigned long g_uploadDeadline = 0;

static bool resetWatchdog()
{
  return esp_task_wdt_reset() == ESP_OK;
}

static bool configureWatchdog()
{
  esp_task_wdt_config_t config = {
      .timeout_ms = TASK_WDT_TIMEOUT_MS,
      .idle_core_mask = 0,
      .trigger_panic = true,
  };

  esp_err_t result = esp_task_wdt_reconfigure(&config);
  if (result == ESP_ERR_INVALID_STATE)
  {
    result = esp_task_wdt_init(&config);
  }
  if (result != ESP_OK)
  {
    Serial.printf("Task watchdog setup failed: %s\n", esp_err_to_name(result));
    return false;
  }

  result = esp_task_wdt_status(NULL);
  if (result == ESP_ERR_NOT_FOUND)
  {
    result = esp_task_wdt_add(NULL);
  }
  if (result != ESP_OK)
  {
    Serial.printf("Task watchdog subscription failed: %s\n", esp_err_to_name(result));
    return false;
  }

  return true;
}

static void scheduleRestart(const char *reason)
{
  if (!g_restartPending)
  {
    Serial.printf("Restart scheduled: %s\n", reason);
    g_restartPending = true;
    g_restartAt = millis() + 500;
  }
}

static void closePanelTransaction(bool sleepPanel)
{
  if (!g_uploadActive)
  {
    return;
  }

  if (sleepPanel && !EPD_Sleep())
  {
    g_uploadResult = UPLOAD_PANEL_FAILURE;
    EPD_ForceSleep();
  }
  else if (!sleepPanel)
  {
    EPD_ForceSleep();
  }

  SPI.endTransaction();
  g_uploadActive = false;
}

static void checkUploadTimeout()
{
  if (!g_uploadActive)
  {
    return;
  }
  if ((long)(millis() - g_uploadDeadline) < 0)
  {
    return;
  }

  Serial.println("Upload transaction timed out");
  g_uploadResult = UPLOAD_ABORTED;
  g_uploadComplete = true;
  closePanelTransaction(false);
  scheduleRestart("upload transaction timed out");
}

static bool beginPanelWrite()
{
  SPI.beginTransaction(SPISettings(10000000, MSBFIRST, SPI_MODE0));
  g_uploadActive = true;

  if (!EPD_Init())
  {
    return false;
  }
  resetWatchdog();
  if (!EPD_Display_White())
  {
    return false;
  }
  resetWatchdog();
  if (!EPD_Sleep())
  {
    return false;
  }
  resetWatchdog();
  if (!EPD_Init())
  {
    return false;
  }
  resetWatchdog();

  EPD_W21_WriteCMD(DTM);
  return true;
}

static bool refreshPanel()
{
  EPD_W21_WriteCMD(PON);
  if (!lcd_chkstatus())
  {
    return false;
  }
  resetWatchdog();

  EPD_W21_WriteCMD(BTST2);
  EPD_W21_WriteDATA(0x6F);
  EPD_W21_WriteDATA(0x1F);
  EPD_W21_WriteDATA(0x17);
  EPD_W21_WriteDATA(0x27);
  EPD_W21_WriteCMD(DRF);
  EPD_W21_WriteDATA(0x00);
  if (!lcd_chkstatus())
  {
    return false;
  }
  resetWatchdog();
  return true;
}

static void startUpload()
{
  Serial.println("Upload start");
  g_bytesReceived = 0;
  g_uploadComplete = false;
  g_uploadResult = UPLOAD_IN_PROGRESS;
  g_uploadDeadline = millis() + UPLOAD_TIMEOUT_MS;

  if (!beginPanelWrite())
  {
    Serial.println("Panel initialization failed");
    g_uploadResult = UPLOAD_PANEL_FAILURE;
    closePanelTransaction(false);
  }
}

static void writeUploadData(const uint8_t *data, size_t size)
{
  if (!g_uploadActive || g_uploadResult != UPLOAD_IN_PROGRESS)
  {
    return;
  }

  const size_t remaining = IMAGE_SIZE - g_bytesReceived;
  const size_t bytesToWrite = min(size, remaining);
  for (size_t i = 0; i < bytesToWrite; ++i)
  {
    EPD_W21_WriteDATA(data[i]);
  }
  g_bytesReceived += bytesToWrite;
  g_uploadDeadline = millis() + UPLOAD_TIMEOUT_MS;

  if (size > bytesToWrite)
  {
    g_uploadResult = UPLOAD_TOO_LARGE;
    Serial.printf("Upload exceeds expected image size of %u bytes\n", (unsigned)IMAGE_SIZE);
  }
}

static void finishUpload()
{
  if (g_uploadResult == UPLOAD_NONE)
  {
    return;
  }

  if (g_uploadActive)
  {
    if (g_uploadResult == UPLOAD_IN_PROGRESS && g_bytesReceived != IMAGE_SIZE)
    {
      g_uploadResult = UPLOAD_SIZE_MISMATCH;
    }
    if (g_uploadResult == UPLOAD_IN_PROGRESS && !refreshPanel())
    {
      g_uploadResult = UPLOAD_PANEL_FAILURE;
    }

    closePanelTransaction(g_uploadResult == UPLOAD_IN_PROGRESS);
    if (g_uploadResult == UPLOAD_IN_PROGRESS)
    {
      g_uploadResult = UPLOAD_SUCCESS;
    }
  }
  g_uploadComplete = true;
  Serial.printf("Upload end, received %u bytes\n", (unsigned)g_bytesReceived);
}

static void abortUpload()
{
  if (g_uploadResult != UPLOAD_NONE)
  {
    Serial.println("Upload aborted by client");
    g_uploadResult = UPLOAD_ABORTED;
    closePanelTransaction(false);
    g_uploadComplete = true;
  }
}

void handleUploadData()
{
  const String contentType = server.header("Content-Type");
  if (contentType.startsWith("multipart/form-data"))
  {
    HTTPUpload &upload = server.upload();
    if (upload.status == UPLOAD_FILE_START)
    {
      startUpload();
    }
    else if (upload.status == UPLOAD_FILE_WRITE)
    {
      writeUploadData(upload.buf, upload.currentSize);
    }
    else if (upload.status == UPLOAD_FILE_END)
    {
      finishUpload();
    }
    else if (upload.status == UPLOAD_FILE_ABORTED)
    {
      abortUpload();
    }
    return;
  }

  if (contentType.startsWith("application/octet-stream"))
  {
    HTTPRaw &raw = server.raw();
    if (raw.status == RAW_START)
    {
      startUpload();
    }
    else if (raw.status == RAW_WRITE)
    {
      writeUploadData(raw.buf, raw.currentSize);
    }
    else if (raw.status == RAW_END)
    {
      finishUpload();
    }
    else if (raw.status == RAW_ABORTED)
    {
      abortUpload();
    }
  }
}

void handleUploadDone()
{
  const String contentType = server.header("Content-Type");
  if (!contentType.startsWith("multipart/form-data") && !contentType.startsWith("application/octet-stream"))
  {
    server.send(415, "text/plain", "Use multipart/form-data or application/octet-stream.\n");
    return;
  }
  if (!g_uploadComplete)
  {
    server.send(400, "text/plain", "Upload did not complete.\n");
    return;
  }

  switch (g_uploadResult)
  {
  case UPLOAD_SUCCESS:
    g_lastSuccessfulUpload = millis();
    server.send(200, "text/plain", "OK\n");
    return;
  case UPLOAD_SIZE_MISMATCH:
  {
    char message[72];
    snprintf(message, sizeof(message), "Size mismatch: got %u, expected %u\n", (unsigned)g_bytesReceived, (unsigned)IMAGE_SIZE);
    server.send(400, "text/plain", message);
    return;
  }
  case UPLOAD_TOO_LARGE:
    server.send(413, "text/plain", "Image is larger than the panel buffer.\n");
    return;
  case UPLOAD_PANEL_FAILURE:
    server.send(503, "text/plain", "Panel operation failed; receiver is restarting.\n");
    scheduleRestart("panel operation failed");
    return;
  case UPLOAD_ABORTED:
    server.send(408, "text/plain", "Upload aborted.\n");
    return;
  default:
    server.send(400, "text/plain", "No upload received.\n");
    return;
  }
}

void handleHealth()
{
  char message[256];
  const unsigned long lastUploadAge = millis() - g_lastSuccessfulUpload;
  snprintf(
      message,
      sizeof(message),
      "heap=%u\nmin_heap=%u\nrssi=%d\nlast_upload_age_ms=%lu\nreset_reason=%d\n",
      ESP.getFreeHeap(),
      ESP.getMinFreeHeap(),
      WiFi.RSSI(),
      lastUploadAge,
      (int)esp_reset_reason());
  server.send(200, "text/plain", message);
}

static void startNetworkServices()
{
  if (g_networkServicesStarted && WiFi.status() == WL_CONNECTED)
  {
    server.stop();
    ArduinoOTA.end();
    MDNS.end();
  }

  if (!MDNS.begin(OTA_HOSTNAME))
  {
    Serial.println("mDNS failed to start");
  }

  ArduinoOTA.begin();
  server.begin();
  g_networkServicesStarted = true;
  Serial.printf("Network services ready, IP: %s\n", WiFi.localIP().toString().c_str());
}

static void checkWifi()
{
  const bool connected = WiFi.status() == WL_CONNECTED;
  if (connected)
  {
    if (!g_wifiWasConnected)
    {
      g_wifiWasConnected = true;
      g_wifiLostSince = 0;
      startNetworkServices();
    }
    return;
  }

  if (g_wifiWasConnected)
  {
    g_wifiWasConnected = false;
    Serial.println("Wi-Fi disconnected");
  }
  if (g_wifiLostSince == 0)
  {
    g_wifiLostSince = millis();
  }
  if (millis() - g_lastWifiReconnect >= WIFI_RECONNECT_INTERVAL_MS)
  {
    g_lastWifiReconnect = millis();
    Serial.println("Requesting Wi-Fi reconnect");
    WiFi.reconnect();
  }
  if (millis() - g_wifiLostSince >= WIFI_REBOOT_TIMEOUT_MS)
  {
    scheduleRestart("Wi-Fi unavailable for five minutes");
  }
}

void setup()
{
  Serial.begin(115200);
  delay(200);
  Serial.println("\n=== GoodDisplay 4in E6 HTTP Frame Receiver ===");
  Serial.printf("Reset reason: %d\n", (int)esp_reset_reason());

  if (!configureWatchdog())
  {
    delay(100);
    ESP.restart();
  }

  gpio_set();
  SPI.begin(EPD_SCK_PIN, -1, EPD_MOSI_PIN, EPD_CS_PIN);
  SPI.beginTransaction(SPISettings(10000000, MSBFIRST, SPI_MODE0));
  EPD_ForceSleep();
  SPI.endTransaction();

  pinMode(WIFI_ENABLE, OUTPUT);
  digitalWrite(WIFI_ENABLE, LOW);
  pinMode(WIFI_ANT_CONFIG, OUTPUT);
  digitalWrite(WIFI_ANT_CONFIG, HIGH);

  WiFi.persistent(false);
  WiFi.mode(WIFI_STA);
  WiFi.config(INADDR_NONE, INADDR_NONE, INADDR_NONE, INADDR_NONE);
  WiFi.setHostname(OTA_HOSTNAME);
  WiFi.setAutoReconnect(true);
  WiFi.begin(ssid, password);

  ArduinoOTA.setHostname(OTA_HOSTNAME);
  ArduinoOTA.setPassword(OTA_PASSWORD);
  ArduinoOTA.onStart([]()
                     { Serial.printf("Start OTA update: %s\n", ArduinoOTA.getCommand() == U_FLASH ? "sketch" : "filesystem"); });
  ArduinoOTA.onProgress([](unsigned int, unsigned int)
                        { resetWatchdog(); });
  ArduinoOTA.onError([](ota_error_t error)
                     { Serial.printf("OTA error: %u\n", (unsigned)error); });

  const char *headers[] = {"Content-Type"};
  server.collectHeaders(headers, 1);
  server.on("/upload", HTTP_POST, handleUploadDone, handleUploadData);
  server.on("/health", HTTP_GET, handleHealth);

  g_lastSuccessfulUpload = millis();
  Serial.printf("Free heap: %u bytes\n", ESP.getFreeHeap());
}

void loop()
{
  resetWatchdog();

  if (g_networkServicesStarted)
  {
    server.handleClient();
    ArduinoOTA.handle();
  }

  checkUploadTimeout();

  if (!g_uploadActive && millis() - g_lastWifiCheck >= WIFI_CHECK_INTERVAL_MS)
  {
    g_lastWifiCheck = millis();
    checkWifi();
  }

  if (!g_uploadActive && WiFi.status() == WL_CONNECTED && millis() - g_lastSuccessfulUpload >= SERVICE_REBOOT_TIMEOUT_MS)
  {
    scheduleRestart("no successful frame upload for three hours");
  }

  if (g_restartPending && !g_uploadActive && millis() - g_restartAt < 0x80000000UL)
  {
    ESP.restart();
  }
}
