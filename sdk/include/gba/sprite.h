/*
 * gba/sprite.h - hardware sprites (called "objects" or OBJ on the GBA)
 *
 * The GBA can show 128 sprites at once, drawn by hardware on top of (or
 * between) the backgrounds. Each sprite is described by 3 16-bit "attributes"
 * stored in OAM (Object Attribute Memory, 0x07000000):
 *
 *   attr0: Y position, shape (square/wide/tall), colour depth, mode
 *   attr1: X position, size, flipping
 *   attr2: first tile index, priority, palette bank
 *
 * The pixels of a sprite are TILES (8x8 blocks) stored in sprite VRAM
 * (0x06010000, "tile_obj_mem"). A 16x16 sprite uses 4 tiles, a 32x32 uses 16.
 * With DCNT_OBJ_1D in REG_DISPCNT, these tiles are simply consecutive.
 *
 * Best practice (used here): keep a copy of OAM in normal RAM ("shadow OAM"),
 * modify it freely during the frame, and copy it to the real OAM during
 * VBlank with oam_update().
 *
 *     oam_init();
 *     sprite_set(0, x, y, SPRITE_16x16, 0, 0);  // sprite #0 uses tiles 0..3
 *     ...
 *     vsync();
 *     oam_update();
 *
 * Don't forget to enable sprites: REG_DISPCNT |= DCNT_OBJ | DCNT_OBJ_1D;
 */
#ifndef GBA_SPRITE_H
#define GBA_SPRITE_H

#include "types.h"

typedef struct {
    u16 attr0;
    u16 attr1;
    u16 attr2;
    s16 fill;       /* used by affine matrices, leave alone */
} ALIGN4 OBJ_ATTR;

#define SPRITE_COUNT    128

/* attr0 */
#define ATTR0_Y(n)      ((n) & 0xFF)
#define ATTR0_REG       0x0000
#define ATTR0_AFF       0x0100      /* affine (rotation/scaling) sprite */
#define ATTR0_HIDE      0x0200      /* hidden (for non-affine sprites) */
#define ATTR0_AFF_DBL   0x0300
#define ATTR0_BLEND     0x0400      /* semi-transparent */
#define ATTR0_WINDOW    0x0800
#define ATTR0_MOSAIC    0x1000
#define ATTR0_4BPP      0x0000
#define ATTR0_8BPP      0x2000
#define ATTR0_SQUARE    0x0000
#define ATTR0_WIDE      0x4000
#define ATTR0_TALL      0x8000
/* attr1 */
#define ATTR1_X(n)      ((n) & 0x1FF)
#define ATTR1_AFF_ID(n) ((n) << 9)
#define ATTR1_HFLIP     0x1000
#define ATTR1_VFLIP     0x2000
#define ATTR1_SIZE(n)   ((n) << 14)
/* attr2 */
#define ATTR2_TILE(n)   ((n) & 0x3FF)
#define ATTR2_PRIO(n)   ((n) << 10)
#define ATTR2_PALBANK(n) ((n) << 12)

/* Sprite sizes: shape and size combined. Use with sprite_set(). */
typedef enum {
    SPRITE_8x8   = 0x00, SPRITE_16x16 = 0x01, SPRITE_32x32 = 0x02, SPRITE_64x64 = 0x03,
    SPRITE_16x8  = 0x04, SPRITE_32x8  = 0x05, SPRITE_32x16 = 0x06, SPRITE_64x32 = 0x07,
    SPRITE_8x16  = 0x08, SPRITE_8x32  = 0x09, SPRITE_16x32 = 0x0A, SPRITE_32x64 = 0x0B,
} SpriteSize;

/* The shadow OAM: modify it, then call oam_update(). */
extern OBJ_ATTR oam_buffer[SPRITE_COUNT];

/* Hide every sprite (in the shadow OAM and in the real OAM). */
void oam_init(void);
/* Copy the shadow OAM to the real OAM. Call right after vsync(). */
void oam_update(void);

/* Configure sprite `id` (0..127) and show it. `tile` is the index of its
 * first 8x8 tile in sprite VRAM; `palbank` the 16-colour palette (0..15). */
OBJ_ATTR *sprite_set(int id, int x, int y, SpriteSize size, int tile, int palbank);
void sprite_move(int id, int x, int y);
void sprite_hide(int id);
void sprite_show(int id);
void sprite_flip(int id, bool hflip, bool vflip);
void sprite_set_tile(int id, int tile);
void sprite_set_priority(int id, int priority);   /* 0 = in front */

/* Width/height in pixels of a SpriteSize. */
int sprite_width(SpriteSize size);
int sprite_height(SpriteSize size);

#endif /* GBA_SPRITE_H */
