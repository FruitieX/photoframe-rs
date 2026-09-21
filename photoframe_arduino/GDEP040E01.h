#ifndef _GDEP040E01_
#define _GDEP040E01_
#include "Arduino.h"
#include "config.h"

// IO settings (adapted for your ESP32 wiring)
// MOSI = GPIO23, SCK = GPIO18, CS = GPIO5, DC = GPIO17, RST = GPIO16, BUSY = GPIO4
// Note: SPI pins are configured via SPI.begin(); these macros control GPIOs.

// #define EPD_BUSY_PIN 4
// #define EPD_RST_PIN 16
// #define EPD_DC_PIN 17
// #define EPD_CS_PIN 5

#define EPD_BUSY_PIN 1
#define EPD_RST_PIN 2
#define EPD_DC_PIN 21
#define EPD_CS_PIN 22
#define EPD_SCK_PIN 23
#define EPD_MOSI_PIN 16

// Display geometry (EPD_WIDTH / EPD_HEIGHT come from config.h)
#define IMAGE_SIZE (EPD_WIDTH * EPD_HEIGHT / 2) // 4bpp packed (2 pixels per byte)

#define isEPD_W21_BUSY digitalRead(EPD_BUSY_PIN)     // BUSY (usually LOW=busy)
#define EPD_W21_RST_0 digitalWrite(EPD_RST_PIN, LOW) // RES
#define EPD_W21_RST_1 digitalWrite(EPD_RST_PIN, HIGH)
#define EPD_W21_DC_0 digitalWrite(EPD_DC_PIN, LOW) // DC
#define EPD_W21_DC_1 digitalWrite(EPD_DC_PIN, HIGH)
#define EPD_W21_CS_0 digitalWrite(EPD_CS_PIN, LOW) // CS
#define EPD_W21_CS_1 digitalWrite(EPD_CS_PIN, HIGH)

#define PSR 0x00
#define PWR 0x01
#define POF 0x02
#define POFS 0x03
#define PON 0x04
#define BTST1 0x05
#define BTST2 0x06
#define DSLP 0x07
#define BTST3 0x08
#define DTM 0x10
#define DRF 0x12
#define PLL 0x30
#define CDI 0x50
#define TCON 0x60
#define TRES 0x61
#define REV 0x70
#define VDCS 0x82
#define T_VDCS 0x84
#define PWS 0xE3

#define Black 0x00
#define White 0x11
#define Green 0x66
#define Blue 0x55
#define Red 0x33
#define Yellow 0x22

void reset(void);
void gpio_set(void);
void SPI_Write(unsigned char value);
void EPD_W21_WriteDATA(unsigned char datas);
void EPD_W21_WriteCMD(unsigned char command);

void EPD_Display(const unsigned char *picData);
// EPD
void EPD_W21_Init(void);
bool EPD_Init();
bool EPD_Sleep(void);
void EPD_ForceSleep(void);

bool EPD_Display_White();
void EPD_Display_Black();
void EPD_Display_Yellow();
void EPD_Display_Red();
void EPD_Display_Blue();
void EPD_Display_Green();

void PIC_display(const unsigned char *picData);
void EPD_sleep(void);
void EPD_refresh(void);
bool lcd_chkstatus(void);
void PIC_display_Clear(void);
void EPD_horizontal(void);
void EPD_vertical(void);
void Acep_color(unsigned char color);

#endif
