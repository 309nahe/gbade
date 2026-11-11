// =============================================================================
//  Sprites - a walking character
// =============================================================================
//
//  Sprites (the GBA calls them "objects") are small images the hardware draws
//  on top of the backgrounds. You don't draw their pixels every frame: you put
//  their tiles in VRAM once, then just tell the hardware where to show them.
//
//  The image comes from assets/hero.16x16.png. GBADE converts it at every
//  build into hero_tiles[] and hero_pal[] (see build/gen/assets.h).
//
//  Controls: D-pad to walk, A to jump.
// =============================================================================

#include <gba.h>
#include <stdio.h>
#include <string.h>
#include "assets.h"     // generated from the PNG files in assets/

#define HERO_SPRITE 0   // which of the 128 sprites we use
#define GROUND_Y    112

int main(void) {
    // Mode 0 with a text layer on BG0 (for the instructions)...
    text_init();
    text_set_background(RGB8(90, 160, 250));
    text_print("\n  Walk with the D-pad,\n  jump with A.");

    // ...plus sprites. DCNT_OBJ_1D means a sprite's tiles are consecutive
    // in memory, which is how GBADE stores sprite sheets.
    REG_DISPCNT |= DCNT_OBJ | DCNT_OBJ_1D;

    // 1. Copy the colours into sprite palette bank 0.
    memcpy(pal_obj_bank(0), hero_pal, HERO_PAL_SIZE);
    // 2. Copy the tiles into sprite tile memory (char block 4).
    memcpy(&tile_mem[4][0], hero_tiles, HERO_TILES_SIZE);
    // 3. Hide all 128 sprites, then set up ours.
    oam_init();
    int x = 112, y = GROUND_Y;
    sprite_set(HERO_SPRITE, x, y, SPRITE_16x16, 0, 0);

    // A simple ground line drawn with a coloured backdrop band would need a
    // background; here the sky colour is enough.
    printf("Hero has %d frames of %d tiles\n", HERO_TILE_COUNT / HERO_FRAME_TILES, HERO_FRAME_TILES);

    int anim_timer = 0, frame = 0;
    fixed vy = 0;               // vertical speed, fixed point (see gba/math.h)
    fixed fy = INT_TO_FIX(y);
    bool facing_left = false;

    while (1) {
        vsync();
        oam_update();           // copy our sprite changes to the hardware NOW,
                                // during VBlank, so there is no tearing
        key_poll();

        // --- Walk -----------------------------------------------------------
        int dx = key_tri_horz();
        x = CLAMP(x + dx * 2, 0, SCREEN_WIDTH - 16);
        if (dx != 0) facing_left = dx < 0;

        // --- Jump with simple gravity ---------------------------------------
        bool on_ground = fy >= INT_TO_FIX(GROUND_Y);
        if (on_ground && key_hit(KEY_A)) {
            vy = FIX(-4.5);
            sound_init();
            sound_beep(NOTE_C5, 6);
        }
        vy += FIX(0.25);        // gravity
        fy += vy;
        if (fy >= INT_TO_FIX(GROUND_Y)) {
            fy = INT_TO_FIX(GROUND_Y);
            vy = 0;
        }
        y = FIX_TO_INT(fy);

        // --- Animation: change frame every 6 frames while walking ------------
        if (dx != 0 && on_ground) {
            if (++anim_timer >= 6) {
                anim_timer = 0;
                frame = (frame + 1) % 4;
            }
        } else if (on_ground) {
            frame = 0;          // standing still
        } else {
            frame = 2;          // in the air
        }

        sprite_move(HERO_SPRITE, x, y);
        sprite_set_tile(HERO_SPRITE, frame * HERO_FRAME_TILES);
        sprite_flip(HERO_SPRITE, facing_left, false);
    }
}
