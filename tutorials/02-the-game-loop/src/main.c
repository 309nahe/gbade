// Tutorial 2: The Game Loop
//
// One turn of the loop per frame: wait for VBlank, update, draw.

#include <gba.h>
#include <stdio.h>

#define SIZE   16
#define SPEED  1        // pixels per frame
#define BACKGROUND RGB15(3, 6, 12)

int main(void) {
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    m3_fill(BACKGROUND);

    int x = 0;
    int y = (SCREEN_HEIGHT - SIZE) / 2;
    u32 shown_seconds = 0xFFFFFFFF;      // what the text on screen says now

    printf("The loop starts.\n");
    while (1) {
        vsync();                         // sleep until VBlank: about 60 times a second

        // Update: erase the square where it was, then move it.
        m3_rect(x, y, SIZE, SIZE, BACKGROUND);
        x += SPEED;
        if (x >= SCREEN_WIDTH) {
            x = -SIZE;                   // wrap around to the left edge
        }

        // Render: draw it where it is now.
        m3_rect(x, y, SIZE, SIZE, CLR_YELLOW);

        // Frames are a clock: 60 of them make about one second.
        // Redraw the text only when it changes: drawing text pixel by pixel
        // is slow, too slow to redo every frame in mode 3.
        u32 seconds = frame_count() / 60;
        if (seconds != shown_seconds) {
            shown_seconds = seconds;
            m3_rect(0, 0, SCREEN_WIDTH, 10, BACKGROUND);
            m3_printf(4, 1, CLR_WHITE, "time: %lu s", (unsigned long)seconds);
            printf("one more second: %lu frames\n", (unsigned long)frame_count());
        }
    }
}
