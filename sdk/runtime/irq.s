@ =============================================================================
@  GBADE interrupt handler
@ =============================================================================
@
@  When an interrupt fires (VBlank, a timer overflow, a key press...), the
@  BIOS switches the CPU to IRQ mode, saves r0-r3, r12 and lr on the IRQ stack,
@  and jumps to the address stored at 0x03007FFC (set up by crt0.s).
@
@  This handler:
@    1. reads which interrupts fired (REG_IF) and are enabled (REG_IE),
@    2. acknowledges them (writing 1 bits back to REG_IF clears them),
@    3. also records them in the BIOS interrupt flags at 0x03007FF8 so that the
@       BIOS functions VBlankIntrWait()/IntrWait() can wake up,
@    4. switches to System mode (so your handler runs on the big main stack
@       instead of the tiny IRQ stack) and calls the C dispatcher
@       `gba_irq_dispatch(flags)` which calls the handlers registered with
@       irq_set_handler().
@
@  It lives in IWRAM (fast 32-bit RAM) and is ARM code, which is the fastest
@  combination on the GBA.
@ =============================================================================

    .syntax unified
    .section .iwram.text, "ax", %progbits
    .arm
    .align 2
    .global gba_irq_handler
gba_irq_handler:
    mov     r2, #0x04000000
    add     r2, r2, #0x200          @ r2 = &REG_IE (0x04000200)
    ldr     r1, [r2]                @ low half = IE, high half = IF
    and     r1, r1, r1, lsr #16     @ r1 = IE & IF  (interrupts to service)
    strh    r1, [r2, #2]            @ REG_IF = r1 -> acknowledge

    ldr     r3, =0x03007FF8         @ BIOS interrupt flags
    ldrh    r0, [r3]
    orr     r0, r0, r1
    strh    r0, [r3]

    @ Is any C handler registered at all? (fast path for the common case)
    ldr     r3, =gba_irq_any_handler
    ldr     r3, [r3]
    cmp     r3, #0
    bxeq    lr

    @ Save IRQ-mode state: SPSR (interrupted code's flags) and lr_irq.
    mrs     r3, spsr
    stmfd   sp!, {r3, lr}

    @ Switch to System mode with IRQs still disabled (no nesting).
    mov     r3, #0x9F
    msr     cpsr_c, r3
    stmfd   sp!, {r3, lr}           @ keep the stack 8-byte aligned (AAPCS)

    mov     r0, r1                  @ argument: the interrupt flags
    ldr     r3, =gba_irq_dispatch
    mov     lr, pc
    bx      r3                      @ call C (Thumb) code

    ldmfd   sp!, {r3, lr}

    @ Back to IRQ mode.
    mov     r3, #0x92
    msr     cpsr_c, r3
    ldmfd   sp!, {r3, lr}
    msr     spsr_cxsf, r3
    bx      lr                      @ return to the BIOS, which returns to the game

    .pool
