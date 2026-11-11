/* gbade_shim.h - see gbade_shim.c */
#ifndef GBADE_SHIM_H
#define GBADE_SHIM_H

#include <stddef.h>
#include <stdint.h>

#define GBADE_STOP_NONE 0
#define GBADE_STOP_BREAKPOINT 1
#define GBADE_STOP_ILLEGAL_OP 2
#define GBADE_STOP_WATCHPOINT 3
#define GBADE_STOP_OTHER 4

typedef struct GbadeEmu GbadeEmu;
typedef void (*gbade_log_fn)(void *ctx, int level, const char *category, const char *message);

GbadeEmu *gbade_emu_create(void);
void gbade_emu_destroy(GbadeEmu *emu);
void gbade_emu_set_log_callback(GbadeEmu *emu, gbade_log_fn fn, void *ctx);
int gbade_emu_load_rom(GbadeEmu *emu, const uint8_t *rom, size_t rom_len, const uint8_t *save, size_t save_len);
void gbade_emu_reset(GbadeEmu *emu);
int gbade_emu_run_frame(GbadeEmu *emu);
int gbade_emu_step(GbadeEmu *emu);
uint32_t gbade_emu_stop_address(GbadeEmu *emu);
uint32_t gbade_emu_frame_counter(GbadeEmu *emu);
const uint32_t *gbade_emu_framebuffer(GbadeEmu *emu);
void gbade_emu_set_keys(GbadeEmu *emu, uint32_t keys);
void gbade_emu_set_audio_rate(GbadeEmu *emu, uint32_t rate);
size_t gbade_emu_read_audio(GbadeEmu *emu, int16_t *out, size_t max_frames);
uint32_t gbade_emu_read(GbadeEmu *emu, uint32_t address, int width);
void gbade_emu_read_block(GbadeEmu *emu, uint32_t address, uint8_t *out, size_t len);
void gbade_emu_write(GbadeEmu *emu, uint32_t address, uint32_t value, int width);
void gbade_emu_cpu_state(GbadeEmu *emu, uint32_t regs[16], uint32_t *cpsr);
int gbade_emu_disassemble(GbadeEmu *emu, uint32_t address, int thumb, char *out, size_t out_len);
int gbade_emu_add_breakpoint(GbadeEmu *emu, uint32_t address);
void gbade_emu_clear_breakpoints(GbadeEmu *emu);
size_t gbade_emu_state_size(GbadeEmu *emu);
int gbade_emu_save_state(GbadeEmu *emu, uint8_t *out, size_t len);
int gbade_emu_load_state(GbadeEmu *emu, const uint8_t *in, size_t len);
size_t gbade_emu_savedata(GbadeEmu *emu, uint8_t *out, size_t max_len);

#endif
