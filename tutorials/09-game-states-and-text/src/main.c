// Tutorial 9: Game States and Text
//
// Title -> Play -> Game Over -> Title. In Play, hit A (X on the keyboard)
// as many times as you can in 10 seconds.

#include <gba.h>
#include <stdio.h>

#define ROUND_FRAMES (10 * 60)   // 10 seconds at 60 frames per second

typedef enum { TITLE, PLAY, GAME_OVER } State;

static State state;
static int score, best, frames_left;

// Every state change goes through here: the place to set the state up.
static void enter(State next) {
    state = next;
    text_clear();
    switch (state) {
    case TITLE:
        text_print_at(10, 6, "HIT FRENZY");
        text_print_at(4, 9, "Hit A as fast as you can");
        text_print_at(9, 12, "PRESS START");
        printf("State: TITLE\n");
        break;
    case PLAY:
        score = 0;
        frames_left = ROUND_FRAMES;
        text_print_at(2, 1, "GO! Hit A!");
        printf("State: PLAY\n");
        break;
    case GAME_OVER:
        if (score > best) best = score;
        text_print_at(10, 6, "GAME OVER");
        text_goto(9, 9);
        text_printf("Score: %d", score);
        text_goto(9, 11);
        text_printf("Best:  %d", best);
        text_print_at(6, 15, "START: back to title");
        printf("State: GAME_OVER, score %d\n", score);
        break;
    }
}

int main(void) {
    text_init();                          // mode 0, a 30x20 text console on BG0
    text_set_background(RGB15(4, 2, 10));
    enter(TITLE);

    while (1) {
        vsync();
        key_poll();

        switch (state) {
        case TITLE:
            if (key_hit(KEY_START)) enter(PLAY);
            break;

        case PLAY:
            if (key_hit(KEY_A)) score++;
            frames_left--;
            // Text is tiles: rewriting it every frame is cheap.
            text_goto(2, 4);
            text_printf("Score: %3d", score);
            text_goto(2, 6);
            text_printf("Time:  %2d s", (frames_left + 59) / 60);
            if (frames_left <= 0) enter(GAME_OVER);
            break;

        case GAME_OVER:
            if (key_hit(KEY_START)) enter(TITLE);
            break;
        }
    }
}
