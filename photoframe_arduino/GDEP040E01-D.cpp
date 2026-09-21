#include "GDEP040E01.h"
#include "Arduino.h"
#include <SPI.h>

static const unsigned long PANEL_BUSY_TIMEOUT_MS = 45000;
void gpio_set(void)
{
	pinMode(EPD_BUSY_PIN, INPUT_PULLUP); // BUSY
	pinMode(EPD_RST_PIN, OUTPUT);	     // RES
	pinMode(EPD_DC_PIN, OUTPUT);	     // DC
	pinMode(EPD_CS_PIN, OUTPUT);	     // CS
	digitalWrite(EPD_CS_PIN, HIGH);
	digitalWrite(EPD_DC_PIN, HIGH);
	digitalWrite(EPD_RST_PIN, HIGH);
}
// SPI write byte
void SPI_Write(unsigned char value)
{
	SPI.transfer(value);
}

// SPI write command
void EPD_W21_WriteCMD(unsigned char command)
{
	EPD_W21_CS_0;
	EPD_W21_DC_0; // D/C#   0:command  1:data
	SPI_Write(command);
	EPD_W21_CS_1;
}
// SPI write data
void EPD_W21_WriteDATA(unsigned char datas)
{
	EPD_W21_CS_0;
	EPD_W21_DC_1; // D/C#   0:command  1:data
	SPI_Write(datas);
	EPD_W21_CS_1;
}

bool lcd_chkstatus(void)
{
	unsigned long start = millis();
	while (!isEPD_W21_BUSY)
	{
		if (millis() - start > PANEL_BUSY_TIMEOUT_MS)
		{
			Serial.printf("EPD busy timeout after %lu ms\n", PANEL_BUSY_TIMEOUT_MS);
			return false;
		}
		delay(10);
	}
	return true;
}

void reset(void)
{
	// 20220330
	// dual reset
	EPD_W21_RST_0; // Reset
	delay(30);
	EPD_W21_RST_1;
	delay(30);
	EPD_W21_RST_0; // Reset
	delay(30);
	EPD_W21_RST_1;
}

bool EPD_Init()
{
	reset();
	if (!lcd_chkstatus())
	{
		return false;
	}
	delay(30);

	// 20211212
	EPD_W21_WriteCMD(0xAA);
	EPD_W21_WriteDATA(0x49);
	EPD_W21_WriteDATA(0x55);
	EPD_W21_WriteDATA(0x20);
	EPD_W21_WriteDATA(0x08);
	EPD_W21_WriteDATA(0x09);
	EPD_W21_WriteDATA(0x18);

	EPD_W21_WriteCMD(PWR);
	EPD_W21_WriteDATA(0x3F);

	EPD_W21_WriteCMD(PSR);
	EPD_W21_WriteDATA(0x5F);
	EPD_W21_WriteDATA(0x69);

	EPD_W21_WriteCMD(BTST1);
	EPD_W21_WriteDATA(0x40);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x2C);

	EPD_W21_WriteCMD(BTST3);
	EPD_W21_WriteDATA(0x6F);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x22);

	// First setting
	EPD_W21_WriteCMD(BTST2);
	EPD_W21_WriteDATA(0x6F);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x17);
	EPD_W21_WriteDATA(0x17);

	EPD_W21_WriteCMD(POFS);
	EPD_W21_WriteDATA(0x00);
	EPD_W21_WriteDATA(0x54);
	EPD_W21_WriteDATA(0x00);
	EPD_W21_WriteDATA(0x44);

	EPD_W21_WriteCMD(TCON);
	EPD_W21_WriteDATA(0x02);
	EPD_W21_WriteDATA(0x00);
	// Please notice that PLL must be set for version 2 IC
	EPD_W21_WriteCMD(PLL);
	EPD_W21_WriteDATA(0x08);

	EPD_W21_WriteCMD(CDI);
	EPD_W21_WriteDATA(0x3F);

	EPD_W21_WriteCMD(TRES);
	EPD_W21_WriteDATA((EPD_WIDTH >> 8) & 0xFF);
	EPD_W21_WriteDATA(EPD_WIDTH & 0xFF);
	EPD_W21_WriteDATA((EPD_HEIGHT >> 8) & 0xFF);
	EPD_W21_WriteDATA(EPD_HEIGHT & 0xFF);

	EPD_W21_WriteCMD(PWS);
	EPD_W21_WriteDATA(0x2F);

	EPD_W21_WriteCMD(T_VDCS);
	EPD_W21_WriteDATA(0x01);
	return true;
}

#define PANEL_PIXELS (EPD_WIDTH * EPD_HEIGHT)
#define PANEL_BYTES (PANEL_PIXELS / 2) // 2 pixels per byte

bool EPD_Display_White()
{
	unsigned long i;

	EPD_W21_WriteCMD(DTM);
	for (i = 0; i < PANEL_BYTES; i++)
	{
		EPD_W21_WriteDATA(0x11);
	}
	EPD_W21_WriteCMD(PON);
	if (!lcd_chkstatus())
	{
		return false;
	}

	// Second setting
	EPD_W21_WriteCMD(BTST2);
	EPD_W21_WriteDATA(0x6F);
	EPD_W21_WriteDATA(0x1F);
	EPD_W21_WriteDATA(0x17);
	EPD_W21_WriteDATA(0x27);

	EPD_W21_WriteCMD(DRF);
	EPD_W21_WriteDATA(0x00);
	return lcd_chkstatus();
}

bool EPD_Sleep(void)
{
	// Power off sequence
	EPD_W21_WriteCMD(POF);
	EPD_W21_WriteDATA(0x00);
	if (!lcd_chkstatus())
	{
		return false;
	}

	// Deep sleep
	EPD_W21_WriteCMD(DSLP);
	EPD_W21_WriteDATA(0xA5);
	return true;
}

void EPD_ForceSleep(void)
{
	reset();
	EPD_W21_WriteCMD(DSLP);
	EPD_W21_WriteDATA(0xA5);
}
