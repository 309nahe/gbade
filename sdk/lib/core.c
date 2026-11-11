/*
 * GBADE library: interrupts, vsync, input, BIOS calls, DMA, timers.
 * See the matching headers in sdk/include/gba/ for explanations.
 */
#include <gba.h>

/* ========================================================================== */
/* Interrupts                                                                  */
/* ========================================================================== */

/* Table of user handlers, read by gba_irq_dispatch (called from irq.s). */
static IrqHandler irq_handlers[IRQ_COUNT];
/* Non-zero when at least one handler is installed (checked by irq.s). */
u32 gba_irq_any_handler;
static volatile u32 frames;

void gba_irq_dispatch(u32 flags) {
    for (int i = 0; i < IRQ_COUNT; i++) {
        if ((flags & (1u << i)) && irq_handlers[i]) {
            irq_handlers[i]();
        }
    }
}

static void count_frames(void) { frames++; }

void irq_enable(IrqIndex irq) {
    REG_IME = 0;
    switch (irq) {
    case IRQ_VBLANK: REG_DISPSTAT |= DSTAT_VBL_IRQ; break;
    case IRQ_HBLANK: REG_DISPSTAT |= DSTAT_HBL_IRQ; break;
    case IRQ_VCOUNT: REG_DISPSTAT |= DSTAT_VCT_IRQ; break;
    default: break;
    }
    REG_IE |= (u16)(1u << irq);
    REG_IME = 1;
}

void irq_disable(IrqIndex irq) {
    REG_IME = 0;
    switch (irq) {
    case IRQ_VBLANK: REG_DISPSTAT &= ~DSTAT_VBL_IRQ; break;
    case IRQ_HBLANK: REG_DISPSTAT &= ~DSTAT_HBL_IRQ; break;
    case IRQ_VCOUNT: REG_DISPSTAT &= ~DSTAT_VCT_IRQ; break;
    default: break;
    }
    REG_IE &= (u16)~(1u << irq);
    REG_IME = 1;
}

void irq_set_handler(IrqIndex irq, IrqHandler handler) {
    if ((unsigned)irq >= IRQ_COUNT) return;
    REG_IME = 0;
    irq_handlers[irq] = handler;
    gba_irq_any_handler = 1;
    REG_IME = 1;
    if (handler) irq_enable(irq);
}

/* ========================================================================== */
/* VSync                                                                       */
/* ========================================================================== */

static bool vsync_ready;

void vsync(void) {
    if (!vsync_ready) {
        vsync_ready = true;
        if (!irq_handlers[IRQ_VBLANK]) {
            irq_handlers[IRQ_VBLANK] = count_frames;
            gba_irq_any_handler = 1;
        }
        irq_enable(IRQ_VBLANK);
    }
    VBlankIntrWait();
}

u32 frame_count(void) { return frames; }

/* ========================================================================== */
/* Input                                                                       */
/* ========================================================================== */

static u16 keys_now, keys_prev;

void key_poll(void) {
    keys_prev = keys_now;
    keys_now = (u16)(~REG_KEYINPUT & KEY_ANY);   /* invert: 1 = pressed */
}
u16  key_held_mask(void)        { return keys_now; }
bool key_held(u16 keys)         { return (keys_now & keys) != 0; }
bool key_hit(u16 keys)          { return (keys_now & ~keys_prev & keys) != 0; }
bool key_released(u16 keys)     { return (~keys_now & keys_prev & keys) != 0; }
int  key_tri_horz(void)         { return ((keys_now >> 4) & 1) - ((keys_now >> 5) & 1); }
int  key_tri_vert(void)         { return ((keys_now >> 7) & 1) - ((keys_now >> 6) & 1); }

/* ========================================================================== */
/* BIOS calls                                                                  */
/* ========================================================================== */
/* This file is compiled as Thumb code, where the SWI number is the 8-bit
 * immediate of the `swi` instruction. */

void VBlankIntrWait(void) {
    __asm__ volatile("swi 0x05" ::: "r0", "r1", "r2", "r3", "memory");
}

void IntrWait(bool discard_old, u16 flags) {
    register u32 r0 __asm__("r0") = discard_old;
    register u32 r1 __asm__("r1") = flags;
    __asm__ volatile("swi 0x04" : "+r"(r0), "+r"(r1) : : "r2", "r3", "memory");
}

void Halt(void) {
    __asm__ volatile("swi 0x02" ::: "r0", "r1", "r2", "r3", "memory");
}

void SoftReset(void) {
    __asm__ volatile("swi 0x00" ::: "memory");
    __builtin_unreachable();
}

s32 Div(s32 num, s32 den) {
    register s32 r0 __asm__("r0") = num;
    register s32 r1 __asm__("r1") = den;
    __asm__ volatile("swi 0x06" : "+r"(r0), "+r"(r1) : : "r2", "r3");
    return r0;
}

s32 DivMod(s32 num, s32 den) {
    register s32 r0 __asm__("r0") = num;
    register s32 r1 __asm__("r1") = den;
    __asm__ volatile("swi 0x06" : "+r"(r0), "+r"(r1) : : "r2", "r3");
    return r1;
}

u32 Sqrt(u32 num) {
    register u32 r0 __asm__("r0") = num;
    __asm__ volatile("swi 0x08" : "+r"(r0) : : "r1", "r2", "r3");
    return r0;
}

u16 ArcTan2(s16 x, s16 y) {
    register s32 r0 __asm__("r0") = x;
    register s32 r1 __asm__("r1") = y;
    __asm__ volatile("swi 0x0A" : "+r"(r0), "+r"(r1) : : "r2", "r3");
    return (u16)r0;
}

void CpuSet(const void *src, void *dst, u32 mode) {
    register const void *r0 __asm__("r0") = src;
    register void *r1 __asm__("r1") = dst;
    register u32 r2 __asm__("r2") = mode;
    __asm__ volatile("swi 0x0B" : "+r"(r0), "+r"(r1), "+r"(r2) : : "r3", "memory");
}

void CpuFastSet(const void *src, void *dst, u32 mode) {
    register const void *r0 __asm__("r0") = src;
    register void *r1 __asm__("r1") = dst;
    register u32 r2 __asm__("r2") = mode;
    __asm__ volatile("swi 0x0C" : "+r"(r0), "+r"(r1), "+r"(r2) : : "r3", "memory");
}

#define BIOS_SRC_DST(name, num)                                                \
    void name(const void *src, void *dst) {                                    \
        register const void *r0 __asm__("r0") = src;                           \
        register void *r1 __asm__("r1") = dst;                                 \
        __asm__ volatile("swi " #num : "+r"(r0), "+r"(r1) : : "r2", "r3", "memory"); \
    }
BIOS_SRC_DST(LZ77UnCompWram, 0x11)
BIOS_SRC_DST(LZ77UnCompVram, 0x12)
BIOS_SRC_DST(RLUnCompWram, 0x14)
BIOS_SRC_DST(RLUnCompVram, 0x15)

/* ========================================================================== */
/* DMA                                                                         */
/* ========================================================================== */

void dma_copy(void *dst, const void *src, u32 bytes) {
    REG_DMA3CNT = 0;
    REG_DMA3SAD = (u32)src;
    REG_DMA3DAD = (u32)dst;
    REG_DMA3CNT = (bytes >> 2) | DMA_32 | DMA_ENABLE;
}

void dma_fill(void *dst, u32 value, u32 bytes) {
    static volatile u32 fill_value;
    fill_value = value;
    REG_DMA3CNT = 0;
    REG_DMA3SAD = (u32)&fill_value;
    REG_DMA3DAD = (u32)dst;
    REG_DMA3CNT = (bytes >> 2) | DMA_32 | DMA_SRC_FIXED | DMA_ENABLE;
}

/* ========================================================================== */
/* Timers (profiling)                                                          */
/* ========================================================================== */

void profile_start(void) {
    REG_TM2CNT = 0;
    REG_TM3CNT = 0;
    REG_TM2D = 0;
    REG_TM3D = 0;
    REG_TM3CNT = TM_ENABLE | TM_CASCADE;
    REG_TM2CNT = TM_ENABLE;
}

u32 profile_stop(void) {
    REG_TM2CNT = 0;
    return ((u32)REG_TM3D << 16) | REG_TM2D;
}
