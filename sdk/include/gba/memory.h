/*
 * gba/memory.h - the GBA memory map
 *
 * Everything on the GBA is "memory mapped": the screen, the palettes, the
 * sprites and even the buttons are just addresses you read or write.
 *
 *   Address      Size    Name     What it is
 *   0x00000000   16 KB   BIOS     built-in system ROM (read-only)
 *   0x02000000   256 KB  EWRAM    external work RAM (slower, big)
 *   0x03000000   32 KB   IWRAM    internal work RAM (fast, small)
 *   0x04000000   1 KB    IO       hardware registers (REG_xxx)
 *   0x05000000   1 KB    PAL      palette RAM: 256 BG colours + 256 sprite colours
 *   0x06000000   96 KB   VRAM     video RAM: tiles, maps, or bitmap pixels
 *   0x07000000   1 KB    OAM      object attribute memory: 128 sprites
 *   0x08000000   32 MB   ROM      your game cartridge
 *   0x0E000000   64 KB   SRAM     battery-backed save memory
 *
 * Important rule: VRAM, palette RAM and OAM can NOT be written one byte at a
 * time (8-bit writes are ignored or duplicated). Always write 16 or 32 bits.
 */
#ifndef GBA_MEMORY_H
#define GBA_MEMORY_H

#include "types.h"

#define MEM_EWRAM   0x02000000
#define MEM_IWRAM   0x03000000
#define MEM_IO      0x04000000
#define MEM_PAL     0x05000000
#define MEM_VRAM    0x06000000
#define MEM_OAM     0x07000000
#define MEM_ROM     0x08000000
#define MEM_SRAM    0x0E000000

#define PAL_SIZE    0x00400
#define VRAM_SIZE   0x18000
#define OAM_SIZE    0x00400

/* --- Palettes ---------------------------------------------------------------
 * Colours are 15-bit: 5 bits each of red, green, blue (0..31).
 * Background palette: 256 colours at 0x05000000.
 * Sprite palette:     256 colours at 0x05000200.
 * In 16-colour (4bpp) mode, each palette is split in 16 "banks" of 16 colours.
 * Colour 0 of each bank is transparent (except BG colour 0 = backdrop). */
#define pal_bg_mem      ((u16 *)MEM_PAL)
#define pal_obj_mem     ((u16 *)(MEM_PAL + 0x200))
#define pal_bg_bank(n)  (pal_bg_mem + (n) * 16)
#define pal_obj_bank(n) (pal_obj_mem + (n) * 16)

/* --- VRAM -------------------------------------------------------------------- */
#define vram            ((u16 *)MEM_VRAM)

/* Bitmap mode 3: one 240x160 screen of 16-bit colours. */
#define m3_mem          ((u16 *)MEM_VRAM)
/* Bitmap mode 4: two 240x160 pages of 8-bit palette indices. */
#define m4_mem          ((u8 *)MEM_VRAM)
#define m4_mem_back     ((u8 *)(MEM_VRAM + 0xA000))
/* Bitmap mode 5: two 160x128 pages of 16-bit colours. */
#define m5_mem          ((u16 *)MEM_VRAM)
#define m5_mem_back     ((u16 *)(MEM_VRAM + 0xA000))

/* Tiles: an 8x8 tile is 32 bytes in 4bpp (16 colours) or 64 in 8bpp.
 * VRAM is divided in 4 "char blocks" of 16 KB for backgrounds (0-3), plus
 * 2 char blocks for sprites (4-5, starting at 0x06010000). */
typedef struct { u32 data[8]; }  TILE;    /* 4bpp tile */
typedef struct { u32 data[16]; } TILE8;   /* 8bpp tile */
typedef TILE  CHARBLOCK[512];
typedef TILE8 CHARBLOCK8[256];
#define tile_mem        ((CHARBLOCK *)MEM_VRAM)
#define tile8_mem       ((CHARBLOCK8 *)MEM_VRAM)
/* Sprite tiles live in char blocks 4 and 5: tile_mem[4][n]. */
#define tile_obj_mem    ((TILE *)(MEM_VRAM + 0x10000))

/* Maps: the same 96 KB also holds "screen blocks" of 2 KB each (0-31).
 * A screen block is a 32x32 array of 16-bit "screen entries":
 *   bits 0-9   tile index
 *   bit  10    horizontal flip
 *   bit  11    vertical flip
 *   bits 12-15 palette bank (4bpp only) */
typedef u16 SCREENBLOCK[1024];
#define se_mem          ((SCREENBLOCK *)MEM_VRAM)
#define SE_HFLIP        0x0400
#define SE_VFLIP        0x0800
#define SE_PALBANK(n)   ((n) << 12)

/* --- OAM ---------------------------------------------------------------------- */
#define oam_mem         ((u16 *)MEM_OAM)

/* --- Save RAM (8-bit access only!) -------------------------------------------- */
#define sram_mem        ((vu8 *)MEM_SRAM)

#endif /* GBA_MEMORY_H */
