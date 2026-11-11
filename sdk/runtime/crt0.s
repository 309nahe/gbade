@ =============================================================================
@  GBADE startup code (crt0) for the Game Boy Advance
@ =============================================================================
@
@  This is the very first code that runs when your game boots. You never need to
@  touch it, but it is heavily commented because understanding it is a great way
@  to learn how the GBA starts up.
@
@  What happens at power on:
@    1. The GBA BIOS runs, shows the logo, checks the cartridge header.
@    2. The BIOS jumps to address 0x08000000 (the first byte of the cartridge
@       ROM) in ARM (32-bit instruction) mode.
@    3. That first word is a branch instruction that jumps over the cartridge
@       header to `start_vector` below.
@
@  What this file does:
@    - reserves space for the 192 byte cartridge header (GBADE fills in the
@      logo, title and checksum when it builds your ROM),
@    - sets up the stacks for IRQ mode and System mode,
@    - copies initialised variables (.data / IWRAM code / EWRAM data) from ROM
@      into RAM, because ROM is read-only,
@    - zeroes uninitialised variables (.bss), as the C standard requires,
@    - installs the interrupt handler,
@    - calls your `main()` function.
@
@  Written for GBADE. Memory map and register details come from GBATEK
@  (Martin Korth) and Tonc (Jasper Vijn); see docs/CREDITS.md.
@ =============================================================================

    .syntax unified
    .section .crt0, "ax", %progbits
    .arm
    .align 2
    .global _start
_start:
    b       start_vector            @ 0x00: jump over the header

    @ 0x04..0x9F: Nintendo logo (156 bytes). Filled in by GBADE's ROM builder.
    .fill   156, 1, 0
    @ 0xA0..0xAB: game title (12 bytes, uppercase ASCII). Filled in by GBADE.
    .fill   12, 1, 0
    @ 0xAC..0xAF: game code (4 bytes). Filled in by GBADE.
    .fill   4, 1, 0
    @ 0xB0..0xB1: maker code (2 bytes).
    .byte   0x30, 0x31
    @ 0xB2: fixed value, must be 0x96.
    .byte   0x96
    @ 0xB3: main unit code (0 = GBA).
    .byte   0x00
    @ 0xB4: device type.
    .byte   0x00
    @ 0xB5..0xBB: reserved, zero.
    .fill   7, 1, 0
    @ 0xBC: software version.
    .byte   0x00
    @ 0xBD: header complement checksum. Computed by GBADE.
    .byte   0x00
    @ 0xBE..0xBF: reserved, zero.
    .fill   2, 1, 0

    @ 0xC0: multiboot entry point (only used when the game is sent over the
    @ link cable). We simply reuse the normal entry point.
    b       start_vector
    .fill   28, 1, 0                @ 0xC4..0xDF multiboot/joybus fields

@ -----------------------------------------------------------------------------
start_vector:
    @ --- Stacks --------------------------------------------------------------
    @ The ARM7TDMI has several CPU "modes", each with its own stack pointer.
    @ We use two: IRQ mode (while handling an interrupt) and System mode
    @ (everything else, including your whole game).
    mov     r0, #0x92               @ IRQ mode, interrupts disabled
    msr     cpsr_c, r0
    ldr     sp, =__sp_irq           @ 0x03007FA0

    mov     r0, #0x1F               @ System mode, interrupts enabled
    msr     cpsr_c, r0
    ldr     sp, =__sp_usr           @ 0x03007F00

    @ --- Copy initialised RAM sections from ROM ------------------------------
    ldr     r0, =__iwram_lma        @ source (in ROM)
    ldr     r1, =__iwram_start      @ destination (in IWRAM)
    ldr     r2, =__iwram_end
    bl      copy_words

    ldr     r0, =__ewram_lma
    ldr     r1, =__ewram_start
    ldr     r2, =__ewram_end
    bl      copy_words

    @ --- Zero .bss (IWRAM) and .sbss (EWRAM) ---------------------------------
    ldr     r1, =__bss_start
    ldr     r2, =__bss_end
    bl      zero_words

    ldr     r1, =__sbss_start
    ldr     r2, =__sbss_end
    bl      zero_words

    @ --- Interrupts -----------------------------------------------------------
    @ The BIOS jumps to the address stored at 0x03007FFC when an interrupt
    @ fires. We point it at our handler (see irq.s).
    ldr     r0, =0x03007FFC
    ldr     r1, =gba_irq_handler
    str     r1, [r0]

    @ --- Call main() ----------------------------------------------------------
    @ main is compiled as Thumb code, so we use `bx` (branch and exchange
    @ instruction set). The ARM7TDMI has no `blx`, hence the `mov lr, pc` trick:
    @ reading pc gives the address of the current instruction + 8, which is
    @ exactly the instruction after `bx`.
    ldr     r0, =main
    mov     lr, pc
    bx      r0

    @ main() returned. A GBA game should never do that; just halt forever.
halt_forever:
    swi     0x020000                @ BIOS Halt (wait for an interrupt)
    b       halt_forever

@ Copy 32-bit words from r0 to r1 until r1 reaches r2.
copy_words:
    cmp     r1, r2
    ldrlo   r3, [r0], #4
    strlo   r3, [r1], #4
    blo     copy_words
    bx      lr

@ Write zeros from r1 until r2.
zero_words:
    mov     r0, #0
1:
    cmp     r1, r2
    strlo   r0, [r1], #4
    blo     1b
    bx      lr

    .pool
