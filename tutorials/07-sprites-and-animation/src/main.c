// Tutorial 7: Sprites and Animation
//
// A 16x16 hero with a two-frame walk, built from text pictures. The arrows
// move it; it animates while walking and turns to face its direction.

#include <gba.h>
#include <stdio.h>

#define FRAME_TIME 8          // frames between two animation frames

// Two 16x16 frames. Each digit is a colour number of the sprite palette.
static const char *walk[2][16] = {
    {
        "................",
        ".....111111.....",
        "....11111111....",
        "....12222221....",
        "....12322321....",
        "....12222221....",
        ".....222222.....",
        "......4444......",
        "....44444444....",
        "...2.444444.2...",
        "...2.444444.2...",
        ".....555555.....",
        ".....55..55.....",
        ".....55..55.....",
        ".....66..66.....",
        "....666..666....",
    },
    {
        "................",
        ".....111111.....",
        "....11111111....",
        "....12222221....",
        "....12322321....",
        "....12222221....",
        ".....222222.....",
        "......4444......",
        "....44444444....",
        "....2444444.2...",
        "...2.444444..2..",
        ".....555555.....",
        "....55....55....",
        "...55......55...",
        "...66......66...",
        "..666......666..",
    },
};

// Store a 16x16 text picture as 4 tiles in 1D order: top-left, top-right,
// bottom-left, bottom-right.
static void make_sprite_tiles(TILE *tiles, const char *pic[16]) {
    for (int t = 0; t < 4; t++) {
        int ox = (t % 2) * 8, oy = (t / 2) * 8;
        for (int y = 0; y < 8; y++) {
            u32 row = 0;
            for (int x = 0; x < 8; x++) {
                char c = pic[oy + y][ox + x];
                u32 colour = c == '.' ? 0 : (u32)(c - '0');
                row |= colour << (x * 4);
            }
            tiles[t].data[y] = row;
        }
    }
}

int main(void) {
    // Mode 0 with sprites on, 1D mapping (a sprite's tiles follow each other).
    REG_DISPCNT = DCNT_MODE0 | DCNT_OBJ | DCNT_OBJ_1D;
    pal_bg_mem[0] = RGB15(10, 18, 28);            // backdrop

    // Sprite palette bank 0 (colour 0 is transparent).
    pal_obj_mem[1] = RGB15(20, 10, 4);            // hair
    pal_obj_mem[2] = RGB15(31, 24, 18);           // skin
    pal_obj_mem[3] = RGB15(2, 2, 6);              // eyes
    pal_obj_mem[4] = RGB15(28, 4, 6);             // shirt
    pal_obj_mem[5] = RGB15(6, 8, 24);             // trousers
    pal_obj_mem[6] = RGB15(8, 4, 2);              // shoes

    // Frame 0 in tiles 0-3, frame 1 in tiles 4-7.
    make_sprite_tiles(&tile_obj_mem[0], walk[0]);
    make_sprite_tiles(&tile_obj_mem[4], walk[1]);

    oam_init();                                    // hide all 128 sprites
    int x = 112, y = 72;
    sprite_set(0, x, y, SPRITE_16x16, 0, 0);       // the hero
    for (int i = 1; i <= 5; i++) {                 // five friends, same tiles
        sprite_set(i, 20 + i * 34, 130, SPRITE_16x16, 0, 0);
    }

    int frame = 0, timer = 0;
    while (1) {
        vsync();
        oam_update();                              // send last frame's changes to OAM
        key_poll();

        int dx = key_tri_horz(), dy = key_tri_vert();
        x = CLAMP(x + dx, 0, SCREEN_WIDTH - 16);
        y = CLAMP(y + dy, 0, SCREEN_HEIGHT - 16);
        sprite_move(0, x, y);

        if (dx != 0) {
            sprite_flip(0, dx < 0, false);         // face left or right
        }
        if (dx != 0 || dy != 0) {
            if (++timer >= FRAME_TIME) {           // next animation frame
                timer = 0;
                frame ^= 1;
                sprite_set_tile(0, frame * 4);
            }
        } else {
            frame = timer = 0;
            sprite_set_tile(0, 0);                 // standing still
        }

        // The friends march in place, all on the same frame.
        int f = (frame_count() / 16) % 2;
        for (int i = 1; i <= 5; i++) sprite_set_tile(i, f * 4);

        if (key_hit(KEY_A)) printf("Hero at (%d, %d)\n", x, y);
    }
}
