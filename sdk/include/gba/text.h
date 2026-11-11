/*
 * gba/text.h - text on screen
 *
 * There is no "print" on a game console: text is just graphics. GBADE comes
 * with a built-in 8x8 pixel font and two ways to show it.
 *
 * 1) Tile text console (mode 0). The screen becomes a 30 x 20 grid of
 *    characters, drawn on a tiled background (BG0 by default). Fast, and you
 *    can still use sprites and other backgrounds:
 *
 *        text_init();
 *        text_print("Hello, GBA!\n");
 *        text_printf("Score: %d\n", score);
 *        text_print_at(10, 5, "PAUSED");
 *
 * 2) Bitmap text (mode 3): draws pixels directly, any position, any colour:
 *
 *        REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
 *        m3_text(20, 40, "Hello!", CLR_YELLOW);
 */
#ifndef GBA_TEXT_H
#define GBA_TEXT_H

#include "types.h"

#define TEXT_COLS 30
#define TEXT_ROWS 20

/* Set up mode 0 with a text console on BG0 (tiles in char block 3, map in
 * screen block 31, palette bank 15). Clears the console. */
void text_init(void);
/* Same but choose the background (0-3), char block (0-3) and screen block
 * (0-31), if your game uses the defaults for something else. Does not change
 * the display mode. */
void text_init_bg(int bg, int charblock, int screenblock);

void text_clear(void);
void text_set_color(u16 color);          /* text colour (default white) */
void text_set_background(u16 color);     /* backdrop colour (default black) */
void text_goto(int col, int row);        /* move the cursor */
void text_putc(char c);
void text_print(const char *s);
void text_print_at(int col, int row, const char *s);
int  text_printf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));

/* Mode 3 bitmap text. (x, y) is the top-left pixel. */
void m3_text(int x, int y, const char *s, u16 color);
int  m3_printf(int x, int y, u16 color, const char *fmt, ...) __attribute__((format(printf, 4, 5)));

/* The raw 8x8 font: 128 characters x 8 rows, bit 0 = leftmost pixel. */
extern const u8 gba_font8x8[128][8];

#endif /* GBA_TEXT_H */
