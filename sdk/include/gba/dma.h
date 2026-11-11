/*
 * gba/dma.h - Direct Memory Access
 *
 * DMA is a hardware unit that copies memory much faster than a CPU loop. The
 * CPU is paused while the transfer runs. There are 4 channels:
 *   DMA0: highest priority, internal memory only
 *   DMA1, DMA2: used for streaming sound to the Direct Sound FIFOs
 *   DMA3: general purpose (the one you'll use for copies)
 *
 *     dma_copy(tile_mem[0], my_tiles, sizeof(my_tiles));
 */
#ifndef GBA_DMA_H
#define GBA_DMA_H

#include "types.h"

typedef struct {
    const void *src;
    void *dst;
    u32 cnt;
} DMA_REC;

#define REG_DMA         ((volatile DMA_REC *)0x040000B0)
#define REG_DMA0SAD     (*(vu32 *)0x040000B0)
#define REG_DMA0DAD     (*(vu32 *)0x040000B4)
#define REG_DMA0CNT     (*(vu32 *)0x040000B8)
#define REG_DMA1SAD     (*(vu32 *)0x040000BC)
#define REG_DMA1DAD     (*(vu32 *)0x040000C0)
#define REG_DMA1CNT     (*(vu32 *)0x040000C4)
#define REG_DMA2SAD     (*(vu32 *)0x040000C8)
#define REG_DMA2DAD     (*(vu32 *)0x040000CC)
#define REG_DMA2CNT     (*(vu32 *)0x040000D0)
#define REG_DMA3SAD     (*(vu32 *)0x040000D4)
#define REG_DMA3DAD     (*(vu32 *)0x040000D8)
#define REG_DMA3CNT     (*(vu32 *)0x040000DC)

#define DMA_DST_INC     0x00000000
#define DMA_DST_DEC     0x00200000
#define DMA_DST_FIXED   0x00400000
#define DMA_DST_RELOAD  0x00600000
#define DMA_SRC_INC     0x00000000
#define DMA_SRC_DEC     0x00800000
#define DMA_SRC_FIXED   0x01000000
#define DMA_REPEAT      0x02000000
#define DMA_16          0x00000000
#define DMA_32          0x04000000
#define DMA_NOW         0x00000000
#define DMA_AT_VBLANK   0x10000000
#define DMA_AT_HBLANK   0x20000000
#define DMA_AT_SPECIAL  0x30000000
#define DMA_IRQ         0x40000000
#define DMA_ENABLE      0x80000000

/* Copy `bytes` bytes (multiple of 4) with DMA3. */
void dma_copy(void *dst, const void *src, u32 bytes);
/* Fill `bytes` bytes (multiple of 4) with a 32-bit value. */
void dma_fill(void *dst, u32 value, u32 bytes);

#endif /* GBA_DMA_H */
