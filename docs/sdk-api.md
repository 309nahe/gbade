# SDK API Reference

Everything is declared by one header:

```c
#include <gba.h>
```

The SDK lives in `sdk/` in this repository and inside the app bundle at
`Contents/Resources/sdk`. This page lists the public API, header by header.
Constants and bit masks are listed in the source headers; the important ones
appear here too.

## gba/types.h

Fixed-size types and macros.

| Name | Meaning |
|---|---|
| `u8 u16 u32 u64` | unsigned 8, 16, 32, 64-bit |
| `s8 s16 s32 s64` | signed versions |
| `vu8 vu16 vu32` | volatile unsigned, for hardware |
| `vs16 vs32` | volatile signed |
| `IWRAM_CODE` | put a function in fast IWRAM as ARM code |
| `EWRAM_DATA` | initialised data in EWRAM |
| `EWRAM_BSS` | zeroed data in EWRAM |
| `ALIGN(n)` `ALIGN4` | alignment attributes |
| `BIT(n)` | `1u << n` |
| `ARRAY_SIZE(a)` | number of elements in an array |
| `MIN` `MAX` `CLAMP` | the obvious helpers |

Also `ARM_CODE`, `THUMB_CODE`, `IWRAM_DATA`, `INLINE`.

## gba/memory.h

Pointers and types for the memory map.

| Name | Address / meaning |
|---|---|
| `MEM_EWRAM` `MEM_IWRAM` `MEM_IO` | region base addresses |
| `MEM_PAL` `MEM_VRAM` `MEM_OAM` | palette, video, sprite memory |
| `MEM_ROM` `MEM_SRAM` | cartridge ROM and save RAM |
| `pal_bg_mem` `pal_obj_mem` | palette arrays |
| `pal_bg_bank(n)` `pal_obj_bank(n)` | 16-colour bank `n` |
| `vram` | VRAM as 16-bit words |
| `m3_mem` | mode 3 bitmap |
| `m4_mem` `m4_mem_back` | mode 4 pages |
| `m5_mem` `m5_mem_back` | mode 5 pages |
| `tile_mem[n]` | character block `n` of VRAM |
| `tile_mem8[n]` | 8bpp character block `n` |
| `se_mem[n]` | screen block `n` |
| `tile_obj_mem` | sprite tiles (`0x06010000`) |
| `oam_mem` `sram_mem` | raw OAM and SRAM |

`TILE` is one 4bpp tile (32 bytes), `TILE8` one 8bpp tile (64 bytes),
`CHARBLOCK` 512 tiles, `SCREENBLOCK` 1024 entries. `SE_HFLIP`,
`SE_VFLIP` and `SE_PALBANK(n)` build map entries.

## gba/video.h

Screen constants, colours and display registers.

| Name | Meaning |
|---|---|
| `SCREEN_WIDTH` `SCREEN_HEIGHT` | 240 and 160 |
| `RGB15(r,g,b)` | colour from 0..31 per channel |
| `RGB8(r,g,b)` | colour from 0..255 per channel |
| `CLR_*` | ready-made colours |
| `REG_DISPCNT` | display control |
| `DCNT_MODE0` .. `DCNT_MODE5` | display mode bits |
| `DCNT_BG0` .. `DCNT_BG3`, `DCNT_OBJ` | enable layers |
| `DCNT_OBJ_1D` | consecutive sprite tiles |
| `DCNT_PAGE` | show the second bitmap page |
| `DCNT_BLANK` | force a white screen |
| `REG_DISPSTAT`, `REG_VCOUNT` | status and current scanline |
| `REG_BG0CNT` .. `REG_BG3CNT` | background control |
| `BG_PRIO` `BG_CBB` `BG_SBB` `BG_4BPP` `BG_8BPP` | control fields |
| `BG_REG_*` | background sizes |
| `REG_BG0HOFS` `REG_BG0VOFS` ... | hardware scroll |
| `REG_BG2PA` ... `REG_BG3Y` | affine parameters |
| `REG_BLDCNT` `REG_BLDALPHA` `REG_BLDY` | blending and fades |
| `REG_MOSAIC` | mosaic |
| `REG_WIN0H` `REG_WIN0V` `REG_WIN1H` `REG_WIN1V` `REG_WININ` `REG_WINOUT` | windows |

Functions:

| Function | Description |
|---|---|
| `void vsync(void)` | wait for VBlank; counts frames; enables the VBlank interrupt |
| `u32 frame_count(void)` | frames since start |
| `void m3_plot(int x, int y, u16 color)` | one mode 3 pixel, bounds-checked |
| `void m3_fill(u16 color)` | fill the screen |
| `void m3_rect(int x, int y, int w, int h, u16 color)` | filled rectangle |
| `void m3_frame(int x, int y, int w, int h, u16 color)` | rectangle outline |
| `void m3_line(int x0, int y0, int x1, int y1, u16 color)` | line |
| `void m3_circle(int cx, int cy, int r, u16 color, bool filled)` | circle |
| `void m4_plot(int x, int y, u8 index)` | mode 4 pixel |
| `void m4_fill(u8 index)` | fill the hidden (drawing) page |
| `void m4_rect(int x, int y, int w, int h, u8 index)` | filled rectangle |
| `u8 *m4_flip(void)` | swap pages, return the new hidden page |

## gba/input.h

| Function | Description |
|---|---|
| `void key_poll(void)` | sample the buttons; call once per frame |
| `u16 key_held_mask(void)` | all buttons held, 1 = pressed |
| `bool key_held(u16 keys)` | any of `keys` is down |
| `bool key_hit(u16 keys)` | any of `keys` went down this frame |
| `bool key_released(u16 keys)` | any of `keys` went up this frame |
| `int key_tri_horz(void)` | -1, 0 or +1 |
| `int key_tri_vert(void)` | -1, 0 or +1 |

Masks: `KEY_A`, `KEY_B`, `KEY_SELECT`, `KEY_START`, `KEY_RIGHT`, `KEY_LEFT`,
`KEY_UP`, `KEY_DOWN`, `KEY_R`, `KEY_L`, `KEY_DPAD`, `KEY_ANY`.

## gba/sprite.h

| Function | Description |
|---|---|
| `void oam_init(void)` | hide all sprites, reset shadow OAM |
| `void oam_update(void)` | copy shadow OAM to the hardware (in VBlank) |
| `OBJ_ATTR *sprite_set(int id, int x, int y, SpriteSize size, int tile, int palbank)` | configure one sprite |
| `void sprite_move(int id, int x, int y)` | set position |
| `void sprite_hide(int id)` / `sprite_show(int id)` | visibility |
| `void sprite_flip(int id, bool hflip, bool vflip)` | flipping |
| `void sprite_set_tile(int id, int tile)` | first tile |
| `void sprite_set_priority(int id, int priority)` | 0 = front |
| `int sprite_width(SpriteSize size)` / `sprite_height(...)` | pixel size |

Sizes: `SPRITE_8x8`, `SPRITE_16x16`, `SPRITE_32x32`, `SPRITE_64x64`,
`SPRITE_16x8`, `SPRITE_32x8`, `SPRITE_32x16`, `SPRITE_64x32`, `SPRITE_8x16`,
`SPRITE_8x32`, `SPRITE_16x32`, `SPRITE_32x64`. The shadow OAM is
`OBJ_ATTR oam_buffer[128]`; the raw attribute macros start with `ATTR0_`,
`ATTR1_`, `ATTR2_`.

## gba/text.h

| Function | Description |
|---|---|
| `void text_init(void)` | mode 0 text console on BG0 |
| `void text_init_bg(int bg, int charblock, int screenblock)` | same, custom placement |
| `void text_clear(void)` | empty the console |
| `void text_set_color(u16 color)` | text colour |
| `void text_set_background(u16 color)` | backdrop colour |
| `void text_goto(int col, int row)` | move the cursor |
| `void text_putc(char c)` | one character |
| `void text_print(const char *s)` | string |
| `void text_print_at(int col, int row, const char *s)` | string at a position |
| `int text_printf(const char *fmt, ...)` | formatted print |
| `void m3_text(int x, int y, const char *s, u16 color)` | mode 3 text |
| `int m3_printf(int x, int y, u16 color, const char *fmt, ...)` | mode 3 formatted text |

The console is 30 columns by 20 rows (`TEXT_COLS`, `TEXT_ROWS`) and handles
`\n`. The font is 8x8, available as `gba_font8x8[128][8]`.

## gba/debug.h

| Function / macro | Description |
|---|---|
| `bool debug_available(void)` | true when a debug output device exists |
| `void debug_log(LogLevel level, const char *fmt, ...)` | one console line with a level |
| `void debug_puts(const char *s)` | raw string |
| `void debug_panic(const char *file, int line, const char *msg)` | stop with a message |
| `ASSERT(cond)` | panic with file and line when false |
| `BREAKPOINT()` | pause here in GBADE |

Levels: `LOG_FATAL`, `LOG_ERROR`, `LOG_WARN`, `LOG_INFO`, `LOG_DEBUG`.

## gba/save.h

| Function | Description |
|---|---|
| `void save_write(u32 offset, const void *data, u32 size)` | write to SRAM |
| `void save_read(u32 offset, void *data, u32 size)` | read from SRAM |
| `void save_erase(void)` | fill SRAM with `0xFF` |

`SAVE_SIZE` is `0x8000` (32 KB). Linking any of these marks the ROM as using
SRAM.

## gba/sound.h

| Function | Description |
|---|---|
| `void sound_init(void)` | enable the sound hardware |
| `void sound_beep(int hz, int frames)` | square beep on channel 1 |
| `void sound_tone(int hz, int frames, int duty)` | square tone on channel 2, duty 0..3 |
| `void sound_noise(int frames, int pitch)` | noise on channel 4, pitch 0..7 |
| `void sound_stop(void)` | silence channels 1, 2 and 4 |

`frames` is in video frames (60 per second); 0 plays until stopped. Notes:
`NOTE_C4` through `NOTE_C5`.

## gba/math.h

| Function / macro | Description |
|---|---|
| `fixed` | 32-bit fixed point, 8 fractional bits |
| `FIX(x)` `INT_TO_FIX(n)` `FIX_TO_INT(f)` `FIX_FRAC(f)` | conversions |
| `fx_mul(a,b)` `fx_div(a,b)` | fixed-point arithmetic |
| `DEG_TO_ANGLE(d)` | degrees to the GBA's 16-bit angle |
| `s32 lu_sin(u32 angle)` / `lu_cos(...)` | sine and cosine from a table, result scaled by 4096 |
| `void rand_seed(u32 seed)` | seed the generator |
| `u32 rand_u32(void)` | next 32-bit value |
| `int rand_range(int min, int max)` | inclusive range |
| `iabs(x)` `isign(x)` | absolute value, sign |

`lu_sin` and `lu_cos` take an angle where a full turn is `0x10000`; the
result is a fixed-point value with 8 fractional bits: 4096 means 16.0, so
multiply by the radius and shift right by 12 to get a pixel offset.

## gba/irq.h

| Name | Description |
|---|---|
| `IrqIndex` | `IRQ_VBLANK`, `IRQ_HBLANK`, `IRQ_VCOUNT`, `IRQ_TIMER0`..`3`, `IRQ_SERIAL`, `IRQ_DMA0`..`3`, `IRQ_KEYPAD`, `IRQ_GAMEPAK` |
| `void irq_set_handler(IrqIndex irq, IrqHandler handler)` | install a handler |
| `void irq_enable(IrqIndex irq)` / `irq_disable(...)` | mask the interrupt |
| `REG_IE` `REG_IF` `REG_IME` `REG_WAITCNT` | interrupt and wait-state registers |

`vsync()` installs its own VBlank handling, so a game that just uses
`vsync()` needs no interrupt code.

## gba/bios.h

Wrappers for BIOS calls.

| Function | Description |
|---|---|
| `void VBlankIntrWait(void)` | sleep until VBlank |
| `void IntrWait(bool discard_old, u16 flags)` | sleep until an interrupt |
| `void Halt(void)` | sleep until any interrupt |
| `void SoftReset(void)` | restart the console |
| `s32 Div(s32 num, s32 den)` | integer division |
| `s32 DivMod(s32 num, s32 den)` | division and remainder |
| `u32 Sqrt(u32 num)` | square root |
| `u16 ArcTan2(s16 x, s16 y)` | angle of a vector |
| `void CpuSet(const void *src, void *dst, u32 mode)` | fast copy or fill |
| `void CpuFastSet(const void *src, void *dst, u32 mode)` | 32-byte-block copy |
| `void LZ77UnCompWram / Vram` | LZ77 decompression |
| `void RLUnCompWram / Vram` | run-length decompression |

`CpuSet` mode uses `CS_FILL` and `CS_32BIT`.

## gba/dma.h

| Function | Description |
|---|---|
| `void dma_copy(void *dst, const void *src, u32 bytes)` | immediate copy |
| `void dma_fill(void *dst, u32 value, u32 bytes)` | fill with a 16-bit value |

Channels are `REG_DMA0SAD` ... `REG_DMA3CNT`; the control bits are
`DMA_DST_INC`, `DMA_SRC_FIXED`, `DMA_16`, `DMA_32`, `DMA_AT_VBLANK`,
`DMA_AT_HBLANK`, `DMA_REPEAT`, `DMA_IRQ`, `DMA_ENABLE`.

## gba/timer.h

Registers `REG_TM0D` ... `REG_TM3CNT`; prescalers `TM_FREQ_1`,
`TM_FREQ_64`, `TM_FREQ_256`, `TM_FREQ_1024`; plus `TM_CASCADE`, `TM_IRQ`,
`TM_ENABLE`.

| Function | Description |
|---|---|
| `void profile_start(void)` | start timer-based measurement |
| `u32 profile_stop(void)` | cycles since `profile_start` |

## libc

The SDK provides a subset of the C library:

- `<stdio.h>`: `printf`, `sprintf`, `snprintf`, `vsnprintf`, `puts`,
  `putchar`.
- `<string.h>`: `memcpy`, `memmove`, `memset`, `memcmp`, `memchr`, `strlen`,
  `strcmp`, `strncmp`, `strcpy`, `strncpy`, `strcat`, `strchr`, `strrchr`,
  `strstr`.
- `<stdlib.h>`: `malloc`, `free`, `calloc`, `realloc`, `rand`, `srand`,
  `atoi`, `strtol`, `qsort`, `abs`, `labs`, `abort`, `exit`.
- `<ctype.h>`, `<assert.h>`, `<stdint.h>`, `<stdbool.h>`, `<stddef.h>`.

There is no file I/O. `%f` works through the software float routines, but it
is slow. The heap is a first-fit allocator in EWRAM.
