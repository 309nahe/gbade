/*
 * GBADE library: bitmap drawing helpers (modes 3 and 4) and sprites.
 */
#include <gba.h>

/* ========================================================================== */
/* Mode 3                                                                      */
/* ========================================================================== */

void m3_fill(u16 color) {
    dma_fill(m3_mem, color | ((u32)color << 16), SCREEN_WIDTH * SCREEN_HEIGHT * 2);
}

void m3_rect(int x, int y, int w, int h, u16 color) {
    int x0 = MAX(x, 0), y0 = MAX(y, 0);
    int x1 = MIN(x + w, SCREEN_WIDTH), y1 = MIN(y + h, SCREEN_HEIGHT);
    for (int yy = y0; yy < y1; yy++) {
        u16 *row = &m3_mem[yy * SCREEN_WIDTH];
        for (int xx = x0; xx < x1; xx++) row[xx] = color;
    }
}

void m3_frame(int x, int y, int w, int h, u16 color) {
    m3_rect(x, y, w, 1, color);
    m3_rect(x, y + h - 1, w, 1, color);
    m3_rect(x, y, 1, h, color);
    m3_rect(x + w - 1, y, 1, h, color);
}

/* Bresenham's line algorithm: only integer additions. */
void m3_line(int x0, int y0, int x1, int y1, u16 color) {
    int dx = iabs(x1 - x0), sx = x0 < x1 ? 1 : -1;
    int dy = -iabs(y1 - y0), sy = y0 < y1 ? 1 : -1;
    int err = dx + dy;
    for (;;) {
        m3_plot(x0, y0, color);
        if (x0 == x1 && y0 == y1) break;
        int e2 = 2 * err;
        if (e2 >= dy) { err += dy; x0 += sx; }
        if (e2 <= dx) { err += dx; y0 += sy; }
    }
}

/* Midpoint circle algorithm. */
void m3_circle(int cx, int cy, int r, u16 color, bool filled) {
    int x = r, y = 0, err = 1 - r;
    while (x >= y) {
        if (filled) {
            m3_rect(cx - x, cy + y, 2 * x + 1, 1, color);
            m3_rect(cx - x, cy - y, 2 * x + 1, 1, color);
            m3_rect(cx - y, cy + x, 2 * y + 1, 1, color);
            m3_rect(cx - y, cy - x, 2 * y + 1, 1, color);
        } else {
            m3_plot(cx + x, cy + y, color); m3_plot(cx - x, cy + y, color);
            m3_plot(cx + x, cy - y, color); m3_plot(cx - x, cy - y, color);
            m3_plot(cx + y, cy + x, color); m3_plot(cx - y, cy + x, color);
            m3_plot(cx + y, cy - x, color); m3_plot(cx - y, cy - x, color);
        }
        y++;
        if (err < 0) {
            err += 2 * y + 1;
        } else {
            x--;
            err += 2 * (y - x) + 1;
        }
    }
}

/* ========================================================================== */
/* Mode 4 (page flipping)                                                      */
/* ========================================================================== */

/* The page we draw into: the one NOT currently displayed. */
static u8 *m4_back = m4_mem_back;

void m4_plot(int x, int y, u8 c) {
    if ((unsigned)x >= SCREEN_WIDTH || (unsigned)y >= SCREEN_HEIGHT) return;
    u16 *p = (u16 *)&m4_back[(y * SCREEN_WIDTH + x) & ~1];
    if (x & 1) *p = (u16)((*p & 0x00FF) | (c << 8));
    else       *p = (u16)((*p & 0xFF00) | c);
}

void m4_fill(u8 c) {
    u32 v = c | (c << 8);
    dma_fill(m4_back, v | (v << 16), SCREEN_WIDTH * SCREEN_HEIGHT);
}

void m4_rect(int x, int y, int w, int h, u8 c) {
    for (int yy = y; yy < y + h; yy++)
        for (int xx = x; xx < x + w; xx++) m4_plot(xx, yy, c);
}

u8 *m4_flip(void) {
    REG_DISPCNT ^= DCNT_PAGE;
    m4_back = (REG_DISPCNT & DCNT_PAGE) ? m4_mem : m4_mem_back;
    return m4_back;
}

/* ========================================================================== */
/* Sprites                                                                     */
/* ========================================================================== */

OBJ_ATTR oam_buffer[SPRITE_COUNT];

void oam_init(void) {
    for (int i = 0; i < SPRITE_COUNT; i++) {
        oam_buffer[i].attr0 = ATTR0_HIDE;
        oam_buffer[i].attr1 = 0;
        oam_buffer[i].attr2 = 0;
    }
    oam_update();
}

void oam_update(void) {
    /* Copy only the 3 attributes, never the `fill` field which belongs to
     * the affine matrices. */
    u16 *dst = oam_mem;
    for (int i = 0; i < SPRITE_COUNT; i++) {
        dst[0] = oam_buffer[i].attr0;
        dst[1] = oam_buffer[i].attr1;
        dst[2] = oam_buffer[i].attr2;
        dst += 4;
    }
}

OBJ_ATTR *sprite_set(int id, int x, int y, SpriteSize size, int tile, int palbank) {
    if ((unsigned)id >= SPRITE_COUNT) return 0;
    OBJ_ATTR *o = &oam_buffer[id];
    u16 shape = (u16)((size >> 2) << 14);
    u16 sz = (u16)((size & 3) << 14);
    o->attr0 = (u16)(ATTR0_Y(y) | shape | ATTR0_4BPP);
    o->attr1 = (u16)(ATTR1_X(x) | sz);
    o->attr2 = (u16)(ATTR2_TILE(tile) | ATTR2_PALBANK(palbank & 15));
    return o;
}

void sprite_move(int id, int x, int y) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    OBJ_ATTR *o = &oam_buffer[id];
    o->attr0 = (u16)((o->attr0 & ~0x00FF) | ATTR0_Y(y));
    o->attr1 = (u16)((o->attr1 & ~0x01FF) | ATTR1_X(x));
}

void sprite_hide(int id) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    oam_buffer[id].attr0 = (u16)((oam_buffer[id].attr0 & ~0x0300) | ATTR0_HIDE);
}

void sprite_show(int id) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    oam_buffer[id].attr0 &= (u16)~0x0300;
}

void sprite_flip(int id, bool hflip, bool vflip) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    u16 a = oam_buffer[id].attr1 & (u16)~(ATTR1_HFLIP | ATTR1_VFLIP);
    if (hflip) a |= ATTR1_HFLIP;
    if (vflip) a |= ATTR1_VFLIP;
    oam_buffer[id].attr1 = a;
}

void sprite_set_tile(int id, int tile) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    oam_buffer[id].attr2 = (u16)((oam_buffer[id].attr2 & ~0x03FF) | ATTR2_TILE(tile));
}

void sprite_set_priority(int id, int prio) {
    if ((unsigned)id >= SPRITE_COUNT) return;
    oam_buffer[id].attr2 = (u16)((oam_buffer[id].attr2 & ~0x0C00) | ATTR2_PRIO(prio & 3));
}

static const u8 sprite_dims[12][2] = {
    {8, 8}, {16, 16}, {32, 32}, {64, 64},
    {16, 8}, {32, 8}, {32, 16}, {64, 32},
    {8, 16}, {8, 32}, {16, 32}, {32, 64},
};
int sprite_width(SpriteSize s)  { return sprite_dims[s % 12][0]; }
int sprite_height(SpriteSize s) { return sprite_dims[s % 12][1]; }
