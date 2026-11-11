/*
 * GBADE library: text rendering with the built-in 8x8 font.
 *
 * The font is "font8x8_basic" by Daniel Hepper (public domain), itself based
 * on the IBM PC VGA font. See docs/CREDITS.md.
 */
#include <gba.h>
#include <stdarg.h>
#include <stdio.h>

/* The font header declares `char font8x8_basic[128][8]`. We rename it and make
 * it const so it stays in ROM instead of using precious RAM. */
#define font8x8_basic gba_font8x8
#define char const u8
#include "../runtime/font/font8x8_basic.h"
#undef char
#undef font8x8_basic

/* ---------------------------------------------------------------------------
 * Tile console
 * ------------------------------------------------------------------------- */

#define TEXT_PALBANK 15

static u16 *text_map;           /* the screen block holding the map */
static int text_col, text_row;
static bool text_ready;

void text_init_bg(int bg, int cbb, int sbb) {
    /* Convert each 1-bit font character to a 4bpp tile: colour index 1 for
     * "ink", 0 (transparent) for the background. 8 pixels x 4 bits = 32 bits
     * per row, pixel 0 in the lowest nibble. */
    TILE *tiles = tile_mem[cbb & 3];
    for (int c = 0; c < 128; c++) {
        for (int row = 0; row < 8; row++) {
            u8 bits = gba_font8x8[c][row];
            u32 line = 0;
            for (int px = 0; px < 8; px++)
                if (bits & (1 << px)) line |= 1u << (px * 4);
            tiles[c].data[row] = line;
        }
    }
    pal_bg_bank(TEXT_PALBANK)[1] = CLR_WHITE;
    REG_BGCNT[bg & 3] = (u16)(BG_CBB(cbb & 3) | BG_SBB(sbb & 31) | BG_4BPP | BG_REG_32x32);
    text_map = se_mem[sbb & 31];
    text_ready = true;
    text_clear();
}

void text_init(void) {
    REG_DISPCNT = DCNT_MODE0 | DCNT_BG0;
    pal_bg_mem[0] = CLR_BLACK;
    text_init_bg(0, 3, 31);
}

void text_clear(void) {
    if (!text_ready) return;
    for (int i = 0; i < 32 * 32; i++) text_map[i] = (u16)(' ' | SE_PALBANK(TEXT_PALBANK));
    text_col = text_row = 0;
}

void text_set_color(u16 color) { pal_bg_bank(TEXT_PALBANK)[1] = color; }
void text_set_background(u16 color) { pal_bg_mem[0] = color; }

void text_goto(int col, int row) {
    text_col = CLAMP(col, 0, TEXT_COLS - 1);
    text_row = CLAMP(row, 0, TEXT_ROWS - 1);
}

static void text_scroll(void) {
    for (int r = 0; r < TEXT_ROWS - 1; r++)
        for (int c = 0; c < TEXT_COLS; c++) text_map[r * 32 + c] = text_map[(r + 1) * 32 + c];
    for (int c = 0; c < TEXT_COLS; c++)
        text_map[(TEXT_ROWS - 1) * 32 + c] = (u16)(' ' | SE_PALBANK(TEXT_PALBANK));
}

void text_putc(char ch) {
    if (!text_ready) text_init();
    if (ch == '\n') {
        text_col = 0;
        text_row++;
    } else if (ch == '\r') {
        text_col = 0;
    } else if (ch == '\t') {
        text_col = (text_col + 4) & ~3;
    } else {
        text_map[text_row * 32 + text_col] = (u16)(((u8)ch & 0x7F) | SE_PALBANK(TEXT_PALBANK));
        text_col++;
    }
    if (text_col >= TEXT_COLS) {
        text_col = 0;
        text_row++;
    }
    if (text_row >= TEXT_ROWS) {
        text_scroll();
        text_row = TEXT_ROWS - 1;
    }
}

void text_print(const char *s) {
    while (*s) text_putc(*s++);
}

void text_print_at(int col, int row, const char *s) {
    text_goto(col, row);
    text_print(s);
}

int text_printf(const char *fmt, ...) {
    char buf[256];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    text_print(buf);
    return n;
}

/* ---------------------------------------------------------------------------
 * Mode 3 bitmap text
 * ------------------------------------------------------------------------- */

void m3_text(int x, int y, const char *s, u16 color) {
    int cx = x;
    for (; *s; s++) {
        if (*s == '\n') {
            cx = x;
            y += 8;
            continue;
        }
        const u8 *glyph = gba_font8x8[(u8)*s & 0x7F];
        for (int row = 0; row < 8; row++) {
            u8 bits = glyph[row];
            for (int px = 0; px < 8; px++)
                if (bits & (1 << px)) m3_plot(cx + px, y + row, color);
        }
        cx += 8;
    }
}

int m3_printf(int x, int y, u16 color, const char *fmt, ...) {
    char buf[256];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    m3_text(x, y, buf, color);
    return n;
}
