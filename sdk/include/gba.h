/*
 * =============================================================================
 *  gba.h  -  the GBADE Game Boy Advance library
 * =============================================================================
 *
 *  Include this one header and you get everything:
 *
 *      #include <gba.h>
 *
 *  It is split into small topic headers you can read to learn how the GBA
 *  hardware works. Each one starts with an explanation:
 *
 *    gba/types.h    basic integer types (u8, u16, s32...) and helper macros
 *    gba/memory.h   the GBA memory map (VRAM, palettes, OAM...)
 *    gba/video.h    display modes, backgrounds, colours, VSync
 *    gba/input.h    reading the buttons
 *    gba/sprite.h   hardware sprites ("objects")
 *    gba/irq.h      interrupts
 *    gba/bios.h     BIOS functions (wait for VBlank, division, sqrt...)
 *    gba/dma.h      DMA: fast memory copies done by hardware
 *    gba/timer.h    hardware timers
 *    gba/sound.h    simple sound effects with the legacy "DMG" channels
 *    gba/math.h     fixed-point numbers, sin/cos, random numbers
 *    gba/text.h     printing text on screen with a built-in font
 *    gba/debug.h    printing to the GBADE console (printf debugging!)
 *    gba/save.h     saving data (SRAM)
 *
 *  The register names follow the conventions used by Tonc
 *  (https://gbadev.net/tonc/), the best GBA tutorial out there, so you can
 *  follow it while using GBADE.
 * =============================================================================
 */
#ifndef GBA_H
#define GBA_H

#include "gba/types.h"
#include "gba/memory.h"
#include "gba/video.h"
#include "gba/input.h"
#include "gba/sprite.h"
#include "gba/irq.h"
#include "gba/bios.h"
#include "gba/dma.h"
#include "gba/timer.h"
#include "gba/sound.h"
#include "gba/math.h"
#include "gba/text.h"
#include "gba/debug.h"
#include "gba/save.h"

#endif /* GBA_H */
