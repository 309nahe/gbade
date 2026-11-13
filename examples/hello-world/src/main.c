// =============================================================================
//  Hello World - your first Game Boy Advance program
// =============================================================================
//
//  Press Cmd+R on macOS, Ctrl+R on Linux (or the Run button) to build and
//  run it.
//
//  What it shows:
//    - printing text on the GBA screen (text_print / text_printf),
//    - printing to the GBADE Console panel (printf) for debugging,
//    - the main loop: vsync() + key_poll() every frame,
//    - reading buttons with key_hit() / key_held().
//
//  Try changing the text, the colours, or what happens when you press A!
// =============================================================================

#include <gba.h>    // everything GBA: video, input, sound, text...
#include <stdio.h>  // printf -> GBADE Console panel

int main(void) {
    // Switch the screen to "mode 0" with a text console on background 0.
    text_init();
    text_set_background(RGB8(20, 30, 60));   // dark blue backdrop
    text_set_color(CLR_WHITE);

    text_print("Hello, Game Boy Advance!\n\n");
    text_print("Press A, B or the D-pad.\n");
    text_print("Press START to clear.\n");

    // printf() does not draw on the screen: it goes to the Console panel at
    // the bottom of GBADE. Perfect for debugging.
    printf("Hello from the GBA! The game has started.\n");

    int presses = 0;

    // A GBA game never returns from main: it loops forever, once per frame
    // (about 60 times per second).
    while (1) {
        vsync();     // wait for the screen to finish drawing (VBlank)
        key_poll();  // read the buttons for this frame

        if (key_hit(KEY_A)) {
            presses++;
            text_printf("A pressed! (%d)\n", presses);
            printf("A was pressed %d time(s)\n", presses);
        }
        if (key_hit(KEY_B)) {
            text_print("B says hi.\n");
        }
        if (key_hit(KEY_DPAD)) {
            // key_tri_horz() is -1 for left, +1 for right, 0 otherwise.
            text_printf("D-pad: x=%d y=%d\n", key_tri_horz(), key_tri_vert());
        }
        if (key_hit(KEY_START)) {
            text_clear();
            text_print("Cleared! Press A again.\n");
        }
    }
}
