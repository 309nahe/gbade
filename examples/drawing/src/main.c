// =============================================================================
//  Drawing - bitmap graphics in Mode 3
// =============================================================================
//
//  In Mode 3, VRAM (0x06000000) holds the screen as 240 x 160 pixels, each a
//  16-bit colour. Writing m3_mem[y * 240 + x] = color; changes one pixel.
//  It is the easiest mode to understand, but also the slowest: the CPU has to
//  draw every pixel itself.
//
//  Controls: D-pad moves the brush, A paints, B changes colour, START clears.
// =============================================================================

#include <gba.h>
#include <stdio.h>

// The palette of brush colours. RGB15 takes red, green, blue from 0 to 31.
static const u16 colors[] = {
    RGB15(31, 31, 31),  // white
    RGB15(31, 8, 8),    // red
    RGB15(31, 24, 0),   // orange
    RGB15(31, 31, 0),   // yellow
    RGB15(8, 28, 8),    // green
    RGB15(8, 16, 31),   // blue
    RGB15(24, 8, 31),   // purple
    RGB15(0, 0, 0),     // black (eraser)
};
#define COLOR_COUNT ARRAY_SIZE(colors)

#define CANVAS_TOP 20                // the top 20 lines are the tool bar
#define BACKGROUND RGB15(3, 4, 8)

// --- The cursor -----------------------------------------------------------
// Mode 3 has no layers: if we draw the cursor into VRAM, it would erase the
// painting. So we remember the 5x5 pixels under the cursor and put them back
// before moving it.
#define CURSOR 5
static u16 under_cursor[CURSOR * CURSOR];

static void save_under(int cx, int cy) {
    for (int y = 0; y < CURSOR; y++)
        for (int x = 0; x < CURSOR; x++)
            under_cursor[y * CURSOR + x] = m3_mem[(cy + y - 2) * SCREEN_WIDTH + (cx + x - 2)];
}

static void restore_under(int cx, int cy) {
    for (int y = 0; y < CURSOR; y++)
        for (int x = 0; x < CURSOR; x++)
            m3_mem[(cy + y - 2) * SCREEN_WIDTH + (cx + x - 2)] = under_cursor[y * CURSOR + x];
}

static void draw_cursor(int cx, int cy, u16 color) {
    // A small "+" shape.
    for (int i = -2; i <= 2; i++) {
        m3_plot(cx + i, cy, color);
        m3_plot(cx, cy + i, color);
    }
}

static void draw_toolbar(int selected) {
    m3_rect(0, 0, SCREEN_WIDTH, CANVAS_TOP, RGB15(6, 6, 10));
    m3_text(4, 6, "Colour:", CLR_WHITE);
    for (int i = 0; i < (int)COLOR_COUNT; i++) {
        int x = 64 + i * 16;
        m3_rect(x, 4, 12, 12, colors[i]);
        if (i == selected) m3_frame(x - 2, 2, 16, 16, CLR_YELLOW);
    }
}

static void draw_demo_shapes(void) {
    // A few shapes to show what the drawing functions do.
    m3_rect(16, 40, 50, 30, RGB15(31, 8, 8));                 // filled rectangle
    m3_frame(80, 40, 50, 30, RGB15(8, 28, 8));                // outline
    m3_line(150, 40, 220, 70, RGB15(31, 31, 0));              // line
    m3_circle(60, 115, 22, RGB15(8, 16, 31), true);           // filled circle
    m3_circle(130, 115, 22, RGB15(24, 8, 31), false);         // circle outline
    m3_text(160, 105, "Paint me!", CLR_WHITE);
}

int main(void) {
    // Mode 3 uses background 2 as the bitmap.
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;

    m3_fill(BACKGROUND);
    draw_demo_shapes();
    int color = 0;
    draw_toolbar(color);

    int x = 120, y = 90;
    save_under(x, y);
    printf("Drawing demo started. Paint with A!\n");

    while (1) {
        vsync();
        key_poll();

        // 1. Remove the cursor from the screen.
        restore_under(x, y);

        // 2. Paint where the brush is.
        if (key_held(KEY_A)) {
            m3_rect(x - 1, y - 1, 3, 3, colors[color]);
        }

        // 3. Move. key_tri_horz() returns -1, 0 or +1.
        x = CLAMP(x + key_tri_horz() * 2, 2, SCREEN_WIDTH - 3);
        y = CLAMP(y + key_tri_vert() * 2, CANVAS_TOP + 2, SCREEN_HEIGHT - 3);

        if (key_hit(KEY_B)) {
            color = (color + 1) % COLOR_COUNT;
            draw_toolbar(color);
            printf("Colour %d selected\n", color);
        }
        if (key_hit(KEY_START)) {
            m3_rect(0, CANVAS_TOP, SCREEN_WIDTH, SCREEN_HEIGHT - CANVAS_TOP, BACKGROUND);
        }

        // 4. Draw the cursor at its new place (remembering what is under it).
        save_under(x, y);
        draw_cursor(x, y, colors[color] == CLR_WHITE ? CLR_YELLOW : CLR_WHITE);
    }
}
