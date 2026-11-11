/* =============================================================================
 *  GBADE linker script for Game Boy Advance cartridge ROMs
 * =============================================================================
 *
 *  A linker script tells the linker WHERE in the GBA's memory each piece of
 *  your program goes. The GBA has three memories you care about:
 *
 *    ROM    0x08000000  up to 32 MB   read-only, slow-ish (16-bit bus)
 *                                     -> code (.text) and constants (.rodata)
 *    IWRAM  0x03000000  32 KB         fast 32-bit RAM inside the CPU
 *                                     -> global variables (.data/.bss), stack,
 *                                        and hot code marked IWRAM_CODE
 *    EWRAM  0x02000000  256 KB        bigger but slower RAM
 *                                     -> data marked EWRAM_DATA / EWRAM_BSS
 *
 *  Initialised variables (int x = 5;) live in RAM but their initial values
 *  must be stored in ROM. That's what "AT> rom" means below: the section RUNS
 *  in RAM (VMA) but is LOADED from ROM (LMA). crt0.s copies it at boot.
 *
 *  The symbols defined here (__iwram_start etc.) are used by crt0.s, and by
 *  GBADE to show you how much memory your game uses.
 * ============================================================================= */

OUTPUT_FORMAT("elf32-littlearm")
OUTPUT_ARCH(arm)
ENTRY(_start)

/* The top 256 bytes of IWRAM hold the stacks' tops and BIOS variables. */
__sp_irq = 0x03007FA0;
__sp_usr = 0x03007F00;

/* Keep at least this much IWRAM free for the main stack. */
__stack_reserve = 0x800; /* 2 KB */

MEMORY {
    rom   (rx)  : ORIGIN = 0x08000000, LENGTH = 32M
    iwram (rwx) : ORIGIN = 0x03000000, LENGTH = 0x7F00 - 0x800
    ewram (rwx) : ORIGIN = 0x02000000, LENGTH = 256K
}

SECTIONS {
    /* ---------------- ROM ---------------- */
    .crt0 : {
        KEEP(*(.crt0))
    } > rom

    .text : {
        *(.text .text.* .gnu.linkonce.t.*)
        *(.glue_7 .glue_7t .v4_bx)
        . = ALIGN(4);
    } > rom

    .rodata : {
        *(.rodata .rodata.* .gnu.linkonce.r.*)
        . = ALIGN(4);
    } > rom

    /* ---------------- IWRAM (copied from ROM at boot) ---------------- */
    .iwram : {
        . = ALIGN(4);
        __iwram_start = .;
        *(.iwram .iwram.* .iwram_code)
        *(.data .data.* .gnu.linkonce.d.*)
        . = ALIGN(4);
        __iwram_end = .;
    } > iwram AT> rom
    __iwram_lma = LOADADDR(.iwram);

    .bss (NOLOAD) : {
        . = ALIGN(4);
        __bss_start = .;
        *(.bss .bss.* .gnu.linkonce.b.* COMMON)
        . = ALIGN(4);
        __bss_end = .;
    } > iwram
    __iwram_top = .;

    /* ---------------- EWRAM ---------------- */
    .ewram : {
        . = ALIGN(4);
        __ewram_start = .;
        *(.ewram.data .ewram.data.* .ewram)
        . = ALIGN(4);
        __ewram_end = .;
    } > ewram AT> rom
    __ewram_lma = LOADADDR(.ewram);
    __rom_end = __ewram_lma + SIZEOF(.ewram);

    .sbss (NOLOAD) : {
        . = ALIGN(4);
        __sbss_start = .;
        *(.ewram.bss .ewram.bss.* .sbss .sbss.*)
        . = ALIGN(4);
        __sbss_end = .;
    } > ewram

    /* Everything after this point in EWRAM is free for malloc(). */
    __heap_start = ALIGN(8);
    __heap_end = 0x02040000;

    /* Exception handling tables are never needed on the GBA. */
    /DISCARD/ : {
        *(.ARM.exidx .ARM.exidx.* .ARM.extab .ARM.extab.*)
        *(.comment .note .note.*)
    }
}
