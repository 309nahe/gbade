// Tutorial 4: Smooth Movement
//
// A bouncing ball with fixed-point position, velocity and gravity.
// A (X on the keyboard) throws it up, the arrows push it sideways.

#include <gba.h>
#include <stdio.h>

#define RADIUS   6
#define GROUND   136
#define GRAVITY  FIX(0.2)       // added to the vertical speed every frame
#define BOUNCE   FIX(0.7)       // keep 70% of the speed after a bounce
#define BACKGROUND RGB15(2, 8, 12)

int main(void) {
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    m3_fill(BACKGROUND);
    // The grass starts just below the lowest pixel the ball can cover.
    m3_rect(0, GROUND + RADIUS + 1, SCREEN_WIDTH, SCREEN_HEIGHT - GROUND - RADIUS - 1, RGB15(6, 14, 4));

    fixed x = INT_TO_FIX(40), y = INT_TO_FIX(20);
    fixed vx = FIX(1.25), vy = 0;
    int bounces = 0;
    m3_printf(4, 1, CLR_WHITE, "bounces: 0");

    while (1) {
        vsync();

        key_poll();

        // Erase the ball where it was drawn last frame.
        m3_circle(FIX_TO_INT(x), FIX_TO_INT(y), RADIUS, BACKGROUND, true);

        // Input: throw up, push sideways.
        if (key_hit(KEY_A)) {
            vy = -FIX(5);
            // y with its fraction: the low 8 bits, counted in 1/256ths.
            printf("Throw from y = %d + %d/256\n", FIX_TO_INT(y), (int)FIX_FRAC(y));
        }
        vx += key_tri_horz() * FIX(0.1);

        // Physics: gravity changes the speed, the speed changes the position.
        vy += GRAVITY;
        x += vx;
        y += vy;

        // Bounce on the ground, losing some energy.
        if (y > INT_TO_FIX(GROUND)) {
            y = INT_TO_FIX(GROUND);
            vy = -fx_mul(vy, BOUNCE);
            if (vy < -FIX(0.5)) {         // a real bounce, not just resting
                bounces++;
                m3_rect(0, 0, SCREEN_WIDTH, 10, BACKGROUND);
                m3_printf(4, 1, CLR_WHITE, "bounces: %d", bounces);
            }
        }
        // Bounce on the walls.
        if (x < INT_TO_FIX(RADIUS) || x > INT_TO_FIX(SCREEN_WIDTH - RADIUS)) {
            vx = -vx;
            x = CLAMP(x, INT_TO_FIX(RADIUS), INT_TO_FIX(SCREEN_WIDTH - RADIUS));
        }

        m3_circle(FIX_TO_INT(x), FIX_TO_INT(y), RADIUS, CLR_YELLOW, true);
    }
}
