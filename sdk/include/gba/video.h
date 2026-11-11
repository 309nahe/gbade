/*
 * gba/video.h - the display
 *
 * The GBA screen is 240 x 160 pixels and refreshes about 59.73 times per
 * second. Drawing one frame takes 228 "scanlines":
 *   - lines 0..159 are drawn on screen (the "VDraw" period),
 *   - lines 160..227 are invisible: the "VBlank" period (about 4.9 ms).
 *
 * Golden rule of GBA games: change video memory during VBlank. If you change
 * it while the screen is being drawn, you get "tearing". So the main loop of
 * almost every game looks like:
 *
 *     while (1) {
 *         vsync();          // sleep until VBlank starts
 *         draw_stuff();     // update VRAM / OAM / scroll registers
 *         game_logic();     // then compute the next frame
 *     }
 *
 * The display has 6 MODES, selected with REG_DISPCNT:
 *   Mode 0: 4 tiled backgrounds (BG0-BG3), no rotation.       <- most games
 *   Mode 1: BG0, BG1 tiled + BG2 affine (rotation/scaling).
 *   Mode 2: BG2, BG3 both affine.
 *   Mode 3: one 240x160 bitmap, 16-bit colour per pixel.       <- easiest!
 *   Mode 4: two 240x160 bitmaps, 8-bit palette index per pixel (page flip).
 *   Mode 5: two 160x128 bitmaps, 16-bit colour (page flip).
 * Sprites (objects) work in every mode.
 */
#ifndef GBA_VIDEO_H
#define GBA_VIDEO_H

#include "types.h"
#include "memory.h"

#define SCREEN_WIDTH    240
#define SCREEN_HEIGHT   160
#define M5_WIDTH        160
#define M5_HEIGHT       128

/* --- Colours --------------------------------------------------------------- */
/* Build a colour from red, green, blue in the range 0..31. */
#define RGB15(r, g, b)  ((u16)((r) | ((g) << 5) | ((b) << 10)))
/* Build a colour from the usual 0..255 range (like in a paint program). */
#define RGB8(r, g, b)   RGB15((r) >> 3, (g) >> 3, (b) >> 3)

#define CLR_BLACK       0x0000
#define CLR_RED         0x001F
#define CLR_LIME        0x03E0
#define CLR_YELLOW      0x03FF
#define CLR_BLUE        0x7C00
#define CLR_MAGENTA     0x7C1F
#define CLR_CYAN        0x7FE0
#define CLR_WHITE       0x7FFF
#define CLR_GRAY        RGB15(16, 16, 16)
#define CLR_ORANGE      RGB15(31, 16, 0)
#define CLR_PURPLE      RGB15(16, 0, 24)
#define CLR_SKY         RGB15(12, 20, 31)

/* --- REG_DISPCNT: display control (0x04000000) --------------------------- */
#define REG_DISPCNT     (*(vu16 *)0x04000000)
#define DCNT_MODE0      0x0000
#define DCNT_MODE1      0x0001
#define DCNT_MODE2      0x0002
#define DCNT_MODE3      0x0003
#define DCNT_MODE4      0x0004
#define DCNT_MODE5      0x0005
#define DCNT_PAGE       0x0010  /* mode 4/5: show the back page */
#define DCNT_OAM_HBL    0x0020  /* allow OAM access during HBlank */
#define DCNT_OBJ_1D     0x0040  /* sprite tiles are stored one after another */
#define DCNT_BLANK      0x0080  /* force the screen white (fast VRAM access) */
#define DCNT_BG0        0x0100  /* enable background 0 */
#define DCNT_BG1        0x0200
#define DCNT_BG2        0x0400
#define DCNT_BG3        0x0800
#define DCNT_OBJ        0x1000  /* enable sprites */
#define DCNT_WIN0       0x2000
#define DCNT_WIN1       0x4000
#define DCNT_WINOBJ     0x8000

/* --- REG_DISPSTAT / REG_VCOUNT --------------------------------------------- */
#define REG_DISPSTAT    (*(vu16 *)0x04000004)
#define DSTAT_IN_VBL    0x0001  /* currently in VBlank */
#define DSTAT_IN_HBL    0x0002  /* currently in HBlank */
#define DSTAT_IN_VCT    0x0004  /* VCOUNT == trigger value */
#define DSTAT_VBL_IRQ   0x0008  /* request an interrupt at VBlank */
#define DSTAT_HBL_IRQ   0x0010
#define DSTAT_VCT_IRQ   0x0020
#define DSTAT_VCT(n)    ((n) << 8)
/* The scanline currently being drawn (0..227). */
#define REG_VCOUNT      (*(vu16 *)0x04000006)

/* --- Backgrounds ------------------------------------------------------------
 * REG_BGxCNT configures tiled background x:
 *   priority (0 = front), char block (where the tiles are), screen block
 *   (where the map is), colour depth, size. */
#define REG_BG0CNT      (*(vu16 *)0x04000008)
#define REG_BG1CNT      (*(vu16 *)0x0400000A)
#define REG_BG2CNT      (*(vu16 *)0x0400000C)
#define REG_BG3CNT      (*(vu16 *)0x0400000E)
#define REG_BGCNT       ((vu16 *)0x04000008)    /* REG_BGCNT[n] */

#define BG_PRIO(n)      (n)
#define BG_CBB(n)       ((n) << 2)  /* char base block 0-3 (tiles) */
#define BG_MOSAIC       0x0040
#define BG_4BPP         0x0000      /* 16 colours per tile (palette banks) */
#define BG_8BPP         0x0080      /* 256 colours per tile */
#define BG_SBB(n)       ((n) << 8)  /* screen base block 0-31 (map) */
#define BG_WRAP         0x2000      /* affine backgrounds: wrap around */
#define BG_SIZE(n)      ((n) << 14)
#define BG_REG_32x32    BG_SIZE(0)  /* 256x256 pixels */
#define BG_REG_64x32    BG_SIZE(1)  /* 512x256 */
#define BG_REG_32x64    BG_SIZE(2)  /* 256x512 */
#define BG_REG_64x64    BG_SIZE(3)  /* 512x512 */

/* Scrolling (write-only!): how many pixels the background is shifted. */
#define REG_BG0HOFS     (*(vu16 *)0x04000010)
#define REG_BG0VOFS     (*(vu16 *)0x04000012)
#define REG_BG1HOFS     (*(vu16 *)0x04000014)
#define REG_BG1VOFS     (*(vu16 *)0x04000016)
#define REG_BG2HOFS     (*(vu16 *)0x04000018)
#define REG_BG2VOFS     (*(vu16 *)0x0400001A)
#define REG_BG3HOFS     (*(vu16 *)0x0400001C)
#define REG_BG3VOFS     (*(vu16 *)0x0400001E)

/* Affine (rotation/scaling) background parameters, 8.8 fixed point. */
#define REG_BG2PA       (*(vs16 *)0x04000020)
#define REG_BG2PB       (*(vs16 *)0x04000022)
#define REG_BG2PC       (*(vs16 *)0x04000024)
#define REG_BG2PD       (*(vs16 *)0x04000026)
#define REG_BG2X        (*(vs32 *)0x04000028)
#define REG_BG2Y        (*(vs32 *)0x0400002C)
#define REG_BG3PA       (*(vs16 *)0x04000030)
#define REG_BG3PB       (*(vs16 *)0x04000032)
#define REG_BG3PC       (*(vs16 *)0x04000034)
#define REG_BG3PD       (*(vs16 *)0x04000036)
#define REG_BG3X        (*(vs32 *)0x04000038)
#define REG_BG3Y        (*(vs32 *)0x0400003C)

/* --- Windows, mosaic, blending ---------------------------------------------- */
#define REG_WIN0H       (*(vu16 *)0x04000040)
#define REG_WIN1H       (*(vu16 *)0x04000042)
#define REG_WIN0V       (*(vu16 *)0x04000044)
#define REG_WIN1V       (*(vu16 *)0x04000046)
#define REG_WININ       (*(vu16 *)0x04000048)
#define REG_WINOUT      (*(vu16 *)0x0400004A)
#define REG_MOSAIC      (*(vu16 *)0x0400004C)
#define REG_BLDCNT      (*(vu16 *)0x04000050)
#define REG_BLDALPHA    (*(vu16 *)0x04000052)
#define REG_BLDY        (*(vu16 *)0x04000054)

#define BLD_BG0         0x0001
#define BLD_BG1         0x0002
#define BLD_BG2         0x0004
#define BLD_BG3         0x0008
#define BLD_OBJ         0x0010
#define BLD_BACKDROP    0x0020
#define BLD_OFF         0x0000
#define BLD_STD         0x0040  /* alpha blending */
#define BLD_WHITE       0x0080  /* fade to white */
#define BLD_BLACK       0x00C0  /* fade to black */
#define BLD_TOP(n)      (n)
#define BLD_BOT(n)      ((n) << 8)

/* --- Helpers (implemented in the GBADE library) ------------------------------ */

/* Wait until the next VBlank. Uses the BIOS so the CPU sleeps (saves battery)
 * instead of spinning. Sets up the VBlank interrupt automatically. */
void vsync(void);

/* Number of frames since the game started (incremented every VBlank). */
u32 frame_count(void);

/* Mode 3 drawing (240x160, 16-bit colour). */
static inline void m3_plot(int x, int y, u16 color) {
    if ((unsigned)x < SCREEN_WIDTH && (unsigned)y < SCREEN_HEIGHT)
        m3_mem[y * SCREEN_WIDTH + x] = color;
}
void m3_fill(u16 color);
void m3_rect(int x, int y, int w, int h, u16 color);
void m3_frame(int x, int y, int w, int h, u16 color);
void m3_line(int x0, int y0, int x1, int y1, u16 color);
void m3_circle(int cx, int cy, int r, u16 color, bool filled);

/* Mode 4 drawing (240x160, 8-bit palette index, double buffered).
 * You can only write 16 bits at a time, so plotting one pixel means
 * read-modify-write of two pixels. */
void m4_plot(int x, int y, u8 color_index);
void m4_fill(u8 color_index);
void m4_rect(int x, int y, int w, int h, u8 color_index);
/* Swap front and back page, returns the page you should now draw into. */
u8 *m4_flip(void);

#endif /* GBA_VIDEO_H */
