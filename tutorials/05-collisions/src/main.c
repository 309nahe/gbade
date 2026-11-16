// Tutorial 5: Collisions
//
// Collect the coins, slide along the walls. Everything is a rectangle.

#include <gba.h>
#include <stdio.h>

#define BACKGROUND RGB15(2, 3, 6)

typedef struct {
    int x, y, w, h;
} Box;

// The classic test: two boxes overlap when they overlap on both axes.
static bool overlap(Box a, Box b) {
    return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

static const Box walls[] = {
    { 60, 30, 10, 90 },
    { 130, 12, 10, 70 },
    { 130, 110, 80, 10 },
    { 0, 10, SCREEN_WIDTH, 2 },   // top border, under the score line
};

static bool hits_a_wall(Box b) {
    for (unsigned i = 0; i < ARRAY_SIZE(walls); i++) {
        if (overlap(b, walls[i])) return true;
    }
    return false;
}

// A new coin somewhere that is not inside a wall.
static Box new_coin(void) {
    Box c = { 0, 0, 8, 8 };
    do {
        c.x = rand_range(0, SCREEN_WIDTH - c.w);
        c.y = rand_range(14, SCREEN_HEIGHT - c.h);
    } while (hits_a_wall(c));
    printf("New coin at (%d, %d)\n", c.x, c.y);
    return c;
}

static void fill(Box b, u16 colour) {
    m3_rect(b.x, b.y, b.w, b.h, colour);
}

int main(void) {
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    m3_fill(BACKGROUND);
    for (unsigned i = 0; i < ARRAY_SIZE(walls); i++) fill(walls[i], CLR_GRAY);

    rand_seed(12345);
    Box player = { 20, 70, 10, 10 };
    Box coin = new_coin();
    int score = 0;
    m3_printf(4, 1, CLR_WHITE, "Score: 0");

    while (1) {
        vsync();
        key_poll();
        fill(player, BACKGROUND);

        // Move, test, undo: one axis at a time, so the player slides.
        int dx = key_tri_horz() * 2, dy = key_tri_vert() * 2;
        player.x += dx;
        if (hits_a_wall(player) || player.x < 0 || player.x > SCREEN_WIDTH - player.w) player.x -= dx;
        player.y += dy;
        if (hits_a_wall(player) || player.y > SCREEN_HEIGHT - player.h) player.y -= dy;

        if (overlap(player, coin)) {
            score++;
            printf("Coin! Score: %d\n", score);
            m3_rect(0, 0, SCREEN_WIDTH, 10, BACKGROUND);   // redraw the score only when it changes
            m3_printf(4, 1, CLR_WHITE, "Score: %d", score);
            fill(coin, BACKGROUND);
            coin = new_coin();
        }

        fill(coin, CLR_YELLOW);
        fill(player, CLR_WHITE);
    }
}
