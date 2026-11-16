// Tutorial 1: First Pixels
//
// Mode 3 turns video memory into a 240x160 picture: one 16-bit colour per
// pixel. Writing to it is all it takes to draw.

#include <gba.h>
#include <stdio.h>

int main(void) {
    // Bitmap mode 3, showing background 2 (the bitmap).
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;

    // Paint the whole screen a dark blue.
    m3_fill(RGB15(2, 4, 10));

    // Three pixels written by hand: pixel (x, y) is m3_mem[y * 240 + x].
    m3_mem[80 * SCREEN_WIDTH + 120] = CLR_RED;
    m3_mem[80 * SCREEN_WIDTH + 136] = CLR_LIME;
    m3_mem[96 * SCREEN_WIDTH + 120] = CLR_BLUE;

    // The same thing with the SDK's helpers.
    m3_rect(20, 20, 40, 30, CLR_YELLOW);          // filled rectangle
    m3_frame(18, 18, 44, 34, CLR_WHITE);          // its outline
    m3_line(0, 159, 239, 110, CLR_ORANGE);        // a line across the bottom
    m3_circle(190, 50, 20, RGB8(255, 105, 180), true);
    m3_text(8, 140, "First pixels!", CLR_WHITE);

    printf("Drew the first pixels.\n");

    // A GBA program never ends: wait for the next frame, forever.
    while (1) {
        vsync();
    }
}
