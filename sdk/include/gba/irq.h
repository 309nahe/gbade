/*
 * gba/irq.h - interrupts
 *
 * An interrupt (IRQ) is the hardware tapping the CPU on the shoulder: "the
 * screen just finished drawing", "a timer overflowed", "a DMA is done".
 * The CPU stops what it was doing, runs a small function (the handler), then
 * resumes. Three switches must ALL be on for an interrupt to reach you:
 *
 *   1. the device must be told to raise it (e.g. DSTAT_VBL_IRQ in REG_DISPSTAT),
 *   2. its bit must be set in REG_IE (interrupt enable),
 *   3. REG_IME (interrupt master enable) must be 1.
 *
 * irq_enable() does all three for you. vsync() automatically enables the
 * VBlank interrupt, so most games never need this file.
 *
 *     void on_vblank(void) { frames++; }
 *     irq_set_handler(IRQ_VBLANK, on_vblank);
 *
 * Handlers run with interrupts disabled: keep them short!
 */
#ifndef GBA_IRQ_H
#define GBA_IRQ_H

#include "types.h"

#define REG_IE          (*(vu16 *)0x04000200)
#define REG_IF          (*(vu16 *)0x04000202)
#define REG_WAITCNT     (*(vu16 *)0x04000204)
#define REG_IME         (*(vu16 *)0x04000208)

typedef enum {
    IRQ_VBLANK = 0, IRQ_HBLANK, IRQ_VCOUNT,
    IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2, IRQ_TIMER3,
    IRQ_SERIAL,
    IRQ_DMA0, IRQ_DMA1, IRQ_DMA2, IRQ_DMA3,
    IRQ_KEYPAD, IRQ_GAMEPAK,
    IRQ_COUNT
} IrqIndex;

typedef void (*IrqHandler)(void);

/* Enable an interrupt source (also sets the device bit for VBlank, HBlank and
 * VCount) and turn on the master switch. */
void irq_enable(IrqIndex irq);
void irq_disable(IrqIndex irq);
/* Register a function called whenever `irq` fires (NULL to remove).
 * This also enables the interrupt. */
void irq_set_handler(IrqIndex irq, IrqHandler handler);

#endif /* GBA_IRQ_H */
