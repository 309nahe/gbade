// Tutorial 3: Buttons
//
// Arrows move the square, A (X on the keyboard) changes its colour,
// holding B (Z) makes it faster.

#include <gba.h>
#include <stdio.h>

#define SIZE 12
#define BACKGROUND RGB15(4, 4, 8)

static const u16 colours[] = { CLR_YELLOW, CLR_CYAN, CLR_MAGENTA, CLR_LIME, CLR_ORANGE };

int main(void) {
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    m3_fill(BACKGROUND);

    int x = 114, y = 74;
    int colour = 0;
    int shown_keys = -1;                  // the register value on screen now

    while (1) {
        vsync();
        key_poll();                       // read the buttons once per frame

        // Erase the square at its old position.
        m3_rect(x, y, SIZE, SIZE, BACKGROUND);

        // Held: move while the arrows are down. B held: faster.
        int speed = key_held(KEY_B) ? 3 : 1;
        x += key_tri_horz() * speed;      // -1, 0 or +1
        y += key_tri_vert() * speed;
        x = CLAMP(x, 0, SCREEN_WIDTH - SIZE);
        y = CLAMP(y, 12, SCREEN_HEIGHT - SIZE);

        // Hit: one colour change per press, however long A is held.
        if (key_hit(KEY_A)) {
            colour = (colour + 1) % ARRAY_SIZE(colours);
            printf("A pressed: colour %d\n", colour);
        }
        if (key_hit(KEY_START)) {
            printf("Start pressed at (%d, %d)\n", x, y);
        }

        m3_rect(x, y, SIZE, SIZE, colours[colour]);

        // The raw register: active-low, 0x03FF when nothing is pressed.
        // Text is slow to draw in mode 3: redraw it only when it changes.
        int keys = REG_KEYINPUT;
        if (keys != shown_keys) {
            shown_keys = keys;
            m3_rect(0, 0, SCREEN_WIDTH, 10, BACKGROUND);
            m3_printf(4, 1, CLR_WHITE, "REG_KEYINPUT = 0x%04X", keys);
        }
    }
}
