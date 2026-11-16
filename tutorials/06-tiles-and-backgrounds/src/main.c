// Tutorial 6: Tiles and Backgrounds
//
// A level made of 8x8 tiles on background 0, in mode 0. The tiles and the
// level are written as text so you can see (and edit) every pixel.

#include <gba.h>
#include <stdio.h>

// One character per pixel: the digit is the colour number (0 = transparent).
static const char *brick[8] = {
    "22222222",
    "11121111",
    "11121111",
    "11121111",
    "22222222",
    "11111112",
    "11111112",
    "11111112",
};
static const char *grass[8] = {
    "33333333",
    "34333433",
    "44444444",
    "55555555",
    "55655556",
    "55555555",
    "56555565",
    "55555555",
};

// The level: 30 columns x 20 rows, exactly one screen.
static const char *level[20] = {
    "..............................",
    "..............................",
    "..............................",
    "..........#####...............",
    "..............................",
    "....................####......",
    "..............................",
    "......###.....................",
    "..............................",
    "..............................",
    "...............#####..........",
    "..............................",
    "..#####.......................",
    "..............................",
    "..........................##..",
    "..............................",
    "==============....============",
    "##############....############",
    "##############....############",
    "##############....############",
};

// Turn 8 strings of digits into a 4bpp tile: one 32-bit word per row,
// pixel 0 in the lowest 4 bits.
static void make_tile(TILE *tile, const char *rows[8]) {
    for (int y = 0; y < 8; y++) {
        u32 row = 0;
        for (int x = 0; x < 8; x++) {
            row |= (u32)(rows[y][x] - '0') << (x * 4);
        }
        tile->data[y] = row;
    }
}

int main(void) {
    // Mode 0: tiled backgrounds. Background 0 takes its tiles from
    // charblock 0 and its map from screenblock 30.
    REG_DISPCNT = DCNT_MODE0 | DCNT_BG0;
    REG_BG0CNT = BG_CBB(0) | BG_SBB(30) | BG_4BPP | BG_REG_32x32;

    // Palette bank 0. Colour 0 of the background palette is the backdrop,
    // shown wherever a tile pixel is transparent: our sky.
    pal_bg_mem[0] = CLR_SKY;
    pal_bg_mem[1] = RGB15(22, 7, 4);    // brick
    pal_bg_mem[2] = RGB15(14, 14, 14);  // mortar
    pal_bg_mem[3] = RGB15(8, 24, 6);    // grass
    pal_bg_mem[4] = RGB15(4, 16, 4);    // dark grass
    pal_bg_mem[5] = RGB15(16, 10, 4);   // dirt
    pal_bg_mem[6] = RGB15(10, 6, 2);    // dark dirt

    // Tile 0 stays empty (all transparent); tiles 1 and 2 are ours.
    make_tile(&tile_mem[0][1], brick);
    make_tile(&tile_mem[0][2], grass);

    // Fill the map from the level text.
    for (int y = 0; y < 20; y++) {
        for (int x = 0; x < 30; x++) {
            char c = level[y][x];
            u16 tile = c == '#' ? 1 : c == '=' ? 2 : 0;
            se_mem[30][y * 32 + x] = tile;
        }
    }

    printf("Level built from %d x %d tiles.\n", 30, 20);
    while (1) {
        vsync();
    }
}
