// Tutorial 8: Scrolling and a Camera
//
// A 512x256 world on a 64x32-tile background. The camera follows the player
// (a sprite) and stops at the edges of the world.

#include <gba.h>
#include <stdio.h>

#define MAP_W   64            // tiles
#define MAP_H   32
#define WORLD_W (MAP_W * 8)   // pixels
#define WORLD_H (MAP_H * 8)
#define SBB     28            // the map uses screenblocks 28 and 29

// Two 8x8 tiles, one 32-bit row each: colour 1 everywhere, then a pattern.
static const TILE ground = { { 0x11111111, 0x12111211, 0x11111111, 0x11211121,
                               0x11111111, 0x12111211, 0x11111111, 0x11211121 } };
static const TILE block  = { { 0x33333333, 0x34444443, 0x34444443, 0x34444443,
                               0x34444443, 0x34444443, 0x34444443, 0x33333333 } };

// Put a tile at map cell (x, y) of a 64x32 map: two 32x32 screenblocks.
static void set_cell(int x, int y, u16 tile) {
    se_mem[SBB + x / 32][y * 32 + x % 32] = tile;
}

int main(void) {
    REG_DISPCNT = DCNT_MODE0 | DCNT_BG0 | DCNT_OBJ | DCNT_OBJ_1D;
    REG_BG0CNT = BG_CBB(0) | BG_SBB(SBB) | BG_4BPP | BG_REG_64x32;

    pal_bg_mem[0] = RGB15(12, 20, 31);    // sky (the backdrop)
    pal_bg_mem[1] = RGB15(10, 16, 6);     // ground
    pal_bg_mem[2] = RGB15(6, 11, 4);
    pal_bg_mem[3] = RGB15(6, 6, 10);      // block border
    pal_bg_mem[4] = RGB15(18, 18, 24);    // block inside
    tile_mem[0][1] = ground;
    tile_mem[0][2] = block;

    // Build the world: ground at the bottom, columns of blocks every 6 tiles,
    // of varying heights, so you can see the world move.
    for (int x = 0; x < MAP_W; x++) {
        for (int y = 0; y < MAP_H; y++) set_cell(x, y, 0);
        for (int y = 26; y < MAP_H; y++) set_cell(x, y, 1);
        if (x % 6 == 3) {
            int height = 2 + (x * 7) % 9;
            for (int y = 26 - height; y < 26; y++) set_cell(x, y, 2);
        }
    }

    // The player: a 16x16 sprite, a plain red square (tiles 0-3, colour 2).
    pal_obj_mem[2] = RGB15(28, 4, 6);
    for (int t = 0; t < 4; t++) {
        for (int row = 0; row < 8; row++) tile_obj_mem[t].data[row] = 0x22222222;
    }
    oam_init();
    sprite_set(0, 0, 0, SPRITE_16x16, 0, 0);

    int px = 40, py = 160;                // player, in world pixels
    int cam_x = 0, cam_y = 0;             // world position of the screen's corner

    while (1) {
        vsync();
        // Write the scroll and sprite position computed last frame.
        REG_BG0HOFS = cam_x;
        REG_BG0VOFS = cam_y;
        sprite_move(0, px - cam_x, py - cam_y);
        oam_update();

        key_poll();
        px = CLAMP(px + key_tri_horz() * 2, 0, WORLD_W - 16);
        py = CLAMP(py + key_tri_vert() * 2, 0, WORLD_H - 16);

        // The camera centres on the player but never leaves the world.
        cam_x = CLAMP(px + 8 - SCREEN_WIDTH / 2, 0, WORLD_W - SCREEN_WIDTH);
        cam_y = CLAMP(py + 8 - SCREEN_HEIGHT / 2, 0, WORLD_H - SCREEN_HEIGHT);

        if (key_hit(KEY_A)) printf("player (%d, %d), camera (%d, %d)\n", px, py, cam_x, cam_y);
    }
}
