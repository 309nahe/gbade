// =============================================================================
//  Scrolling World - tiled backgrounds and hardware scrolling
// =============================================================================
//
//  A tiled background is made of:
//    - TILES: 8x8 pixel images, stored in a "char block" (16 KB of VRAM),
//    - a MAP: a grid of numbers saying which tile goes where, stored in a
//      "screen block" (2 KB of VRAM, 32x32 entries).
//
//  The world image (assets/backgrounds/world.png, 512x256 pixels) is converted
//  by GBADE into world_tiles[] (only the unique tiles!) and world_map[].
//  A 512x256 map is 64x32 tiles = two screen blocks side by side.
//
//  The screen only shows 240x160 pixels: the GBA scrolls the background in
//  hardware, we just write the scroll position to REG_BG0HOFS/VOFS. Free!
//
//  VRAM layout used here:
//    char block 0      world tiles
//    char block 3      text font (text_init_bg)
//    screen blocks 28-29  world map (64x32)
//    screen block 31   text map
//    char block 4      sprite tiles (the hero)
//
//  Controls: D-pad walks (the camera follows), A jumps.
// =============================================================================

#include <gba.h>
#include <stdio.h>
#include <string.h>
#include "assets.h"

#define WORLD_SBB 28
#define WORLD_W   512
#define WORLD_H   256
#define GROUND_Y  176     // feet of the hero in world coordinates

int main(void) {
    // --- Background 0: the world -------------------------------------------
    memcpy(pal_bg_bank(0), world_pal, WORLD_PAL_SIZE);
    memcpy(&tile_mem[0][0], world_tiles, WORLD_TILES_SIZE);
    memcpy(&se_mem[WORLD_SBB][0], world_map, WORLD_MAP_SIZE);
    REG_BG0CNT = BG_CBB(0) | BG_SBB(WORLD_SBB) | BG_4BPP | BG_REG_64x32 | BG_PRIO(1);

    // --- Background 1: a text layer in front --------------------------------
    text_init_bg(1, 3, 31);
    REG_BG1CNT |= BG_PRIO(0);
    text_set_color(CLR_WHITE);

    // --- Sprite: the hero ----------------------------------------------------
    memcpy(pal_obj_bank(0), hero_pal, HERO_PAL_SIZE);
    memcpy(&tile_mem[4][0], hero_tiles, HERO_TILES_SIZE);
    oam_init();
    sprite_set(0, 0, 0, SPRITE_16x16, 0, 0);

    REG_DISPCNT = DCNT_MODE0 | DCNT_BG0 | DCNT_BG1 | DCNT_OBJ | DCNT_OBJ_1D;

    printf("world: %d unique tiles, map %dx%d\n", WORLD_TILE_COUNT, WORLD_MAP_WIDTH, WORLD_MAP_HEIGHT);

    int hero_x = 40;                  // world coordinates
    fixed hero_y = INT_TO_FIX(GROUND_Y - 16), vy = 0;
    int anim = 0, frame = 0;
    bool left = false;

    while (1) {
        vsync();
        oam_update();
        key_poll();

        // Move the hero in the world.
        int dx = key_tri_horz() * 2;
        hero_x = CLAMP(hero_x + dx, 0, WORLD_W - 16);
        if (dx) left = dx < 0;
        bool on_ground = hero_y >= INT_TO_FIX(GROUND_Y - 16);
        if (on_ground && key_hit(KEY_A)) vy = FIX(-5);
        vy += FIX(0.3);
        hero_y += vy;
        if (hero_y > INT_TO_FIX(GROUND_Y - 16)) {
            hero_y = INT_TO_FIX(GROUND_Y - 16);
            vy = 0;
        }

        // The camera follows the hero, but never shows outside the world.
        int cam_x = CLAMP(hero_x + 8 - SCREEN_WIDTH / 2, 0, WORLD_W - SCREEN_WIDTH);
        int cam_y = WORLD_H - SCREEN_HEIGHT;  // bottom of the world
        REG_BG0HOFS = cam_x;
        REG_BG0VOFS = cam_y;

        // Sprites use SCREEN coordinates: world position minus camera.
        if (dx && on_ground) {
            if (++anim >= 6) { anim = 0; frame = (frame + 1) % 4; }
        } else {
            frame = on_ground ? 0 : 2;
        }
        sprite_move(0, hero_x - cam_x, FIX_TO_INT(hero_y) - cam_y);
        sprite_set_tile(0, frame * HERO_FRAME_TILES);
        sprite_flip(0, left, false);

        text_goto(1, 1);
        text_printf("camera x=%3d  hero x=%3d ", cam_x, hero_x);
    }
}
