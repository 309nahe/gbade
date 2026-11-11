// =============================================================================
//  Pong - a complete game in one file
// =============================================================================
//
//  You are the left paddle (D-pad up/down). First to 5 points wins.
//  The longest rally is saved in SRAM, so it survives turning the GBA off.
//
//  Structure of the game:
//    - a small "state machine": TITLE -> PLAYING -> GAME_OVER -> TITLE
//    - every frame: vsync(), read input, update the world, draw what moved
//
//  Drawing: we use Mode 3 and only redraw what moves (erase the old position,
//  draw the new one). Redrawing the full 240x160 screen every frame in Mode 3
//  would be too slow.
// =============================================================================

#include <gba.h>
#include <stdio.h>

#define BG_COLOR     RGB15(2, 3, 6)
#define FG_COLOR     RGB15(28, 30, 31)
#define BALL_COLOR   RGB15(31, 28, 8)
#define PADDLE_W     4
#define PADDLE_H     28
#define BALL_SIZE    5
#define TOP          16      // the score bar uses the top 16 lines
#define WIN_SCORE    5

typedef enum { TITLE, PLAYING, GAME_OVER } State;

typedef struct {
    int x, y;                // top-left corner, pixels
} Paddle;

typedef struct {
    fixed x, y;              // fixed point (24.8), see gba/math.h
    fixed vx, vy;            // speed in pixels per frame, fixed point
} Ball;

typedef struct {
    u32 magic;
    int best_rally;
} SaveData;

#define SAVE_MAGIC 0x504F4E47  // "PONG"

static Paddle player, cpu;
static Ball ball;
static int score_player, score_cpu, rally, best_rally;
static State state;

// --- Helpers ------------------------------------------------------------------

static bool overlap(int ax, int ay, int aw, int ah, int bx, int by, int bw, int bh) {
    return ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
}

static void draw_paddle(const Paddle *p, u16 color) {
    m3_rect(p->x, p->y, PADDLE_W, PADDLE_H, color);
}

static void draw_ball(u16 color) {
    m3_rect(FIX_TO_INT(ball.x), FIX_TO_INT(ball.y), BALL_SIZE, BALL_SIZE, color);
}

static void draw_net(void) {
    for (int y = TOP; y < SCREEN_HEIGHT; y += 8) m3_rect(119, y, 2, 4, RGB15(8, 9, 12));
}

static void draw_scores(void) {
    m3_rect(0, 0, SCREEN_WIDTH, TOP, RGB15(4, 5, 10));
    m3_printf(60, 4, FG_COLOR, "%d", score_player);
    m3_printf(172, 4, FG_COLOR, "%d", score_cpu);
    m3_printf(88, 4, RGB15(14, 16, 20), "best %d", best_rally);
}

static void load_best(void) {
    SaveData s;
    save_read(0, &s, sizeof s);
    best_rally = (s.magic == SAVE_MAGIC) ? s.best_rally : 0;
}

static void store_best(void) {
    SaveData s = { SAVE_MAGIC, best_rally };
    save_write(0, &s, sizeof s);
}

static void serve(int direction) {
    ball.x = INT_TO_FIX(118);
    ball.y = INT_TO_FIX(rand_range(TOP + 20, SCREEN_HEIGHT - 20));
    ball.vx = direction * FIX(1.5);
    ball.vy = rand_range(0, 1) ? FIX(0.75) : FIX(-0.75);
    rally = 0;
}

static void start_match(void) {
    m3_fill(BG_COLOR);
    score_player = score_cpu = 0;
    player = (Paddle){ 8, 66 };
    cpu = (Paddle){ SCREEN_WIDTH - 8 - PADDLE_W, 66 };
    draw_net();
    draw_scores();
    serve(1);
    state = PLAYING;
}

static void show_title(void) {
    m3_fill(BG_COLOR);
    m3_text(104, 50, "PONG", BALL_COLOR);
    m3_text(52, 80, "Press START to play", FG_COLOR);
    m3_text(40, 100, "D-pad up/down: move", RGB15(16, 18, 22));
    m3_printf(72, 130, RGB15(16, 18, 22), "Best rally: %d", best_rally);
    state = TITLE;
}

// --- One frame of gameplay ------------------------------------------------------

static void update_playing(void) {
    // Erase moving things at their old positions.
    draw_paddle(&player, BG_COLOR);
    draw_paddle(&cpu, BG_COLOR);
    draw_ball(BG_COLOR);

    // Player paddle.
    player.y = CLAMP(player.y + key_tri_vert() * 3, TOP, SCREEN_HEIGHT - PADDLE_H);

    // Computer paddle: follows the ball, a bit slower than the player.
    int target = FIX_TO_INT(ball.y) - PADDLE_H / 2;
    if (cpu.y < target - 2) cpu.y += 2;
    else if (cpu.y > target + 2) cpu.y -= 2;
    cpu.y = CLAMP(cpu.y, TOP, SCREEN_HEIGHT - PADDLE_H);

    // Ball.
    ball.x += ball.vx;
    ball.y += ball.vy;
    int bx = FIX_TO_INT(ball.x), by = FIX_TO_INT(ball.y);

    // Bounce on top and bottom walls.
    if (by < TOP || by > SCREEN_HEIGHT - BALL_SIZE) {
        ball.vy = -ball.vy;
        ball.y = INT_TO_FIX(CLAMP(by, TOP, SCREEN_HEIGHT - BALL_SIZE));
        sound_tone(220, 3, 1);
    }

    // Bounce on paddles. The ball goes up or down depending on where it hits.
    Paddle *hit = 0;
    if (ball.vx < 0 && overlap(bx, by, BALL_SIZE, BALL_SIZE, player.x, player.y, PADDLE_W, PADDLE_H)) hit = &player;
    if (ball.vx > 0 && overlap(bx, by, BALL_SIZE, BALL_SIZE, cpu.x, cpu.y, PADDLE_W, PADDLE_H)) hit = &cpu;
    if (hit) {
        int offset = (by + BALL_SIZE / 2) - (hit->y + PADDLE_H / 2);   // -16..16
        ball.vx = -ball.vx;
        // Speed up a little every hit, up to a limit.
        if (iabs(ball.vx) < FIX(4)) ball.vx += ball.vx > 0 ? FIX(0.125) : -FIX(0.125);
        ball.vy = offset * FIX(0.12);
        rally++;
        sound_beep(hit == &player ? NOTE_A4 : NOTE_E4, 4);
    }

    // Points.
    if (bx < -BALL_SIZE || bx > SCREEN_WIDTH) {
        bool player_scored = bx > SCREEN_WIDTH;
        if (player_scored) score_player++; else score_cpu++;
        if (rally > best_rally) {
            best_rally = rally;
            store_best();
            printf("New best rally: %d\n", best_rally);
        }
        printf("Point for %s (%d - %d)\n", player_scored ? "you" : "the CPU", score_player, score_cpu);
        sound_noise(12, 4);
        draw_scores();
        draw_net();
        if (score_player == WIN_SCORE || score_cpu == WIN_SCORE) {
            m3_text(player_scored ? 76 : 80, 70, player_scored ? "YOU WIN!" : "CPU WINS", BALL_COLOR);
            m3_text(48, 90, "Press START to restart", FG_COLOR);
            state = GAME_OVER;
            return;
        }
        serve(player_scored ? -1 : 1);
    }

    // Draw at the new positions.
    if (FIX_TO_INT(ball.x) > 110 && FIX_TO_INT(ball.x) < 126) draw_net();
    draw_paddle(&player, FG_COLOR);
    draw_paddle(&cpu, FG_COLOR);
    draw_ball(BALL_COLOR);
}

int main(void) {
    REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;
    sound_init();
    load_best();
    show_title();

    while (1) {
        vsync();
        key_poll();
        switch (state) {
        case TITLE:
            rand_u32();     // advance the random numbers while waiting: every game is different
            if (key_hit(KEY_START | KEY_A)) start_match();
            break;
        case PLAYING:
            update_playing();
            break;
        case GAME_OVER:
            if (key_hit(KEY_START)) show_title();
            break;
        }
    }
}
