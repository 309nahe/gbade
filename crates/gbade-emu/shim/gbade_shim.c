/*
 * gbade_shim.c - a small, stable C API over mGBA's core, used from Rust.
 *
 * mGBA (https://mgba.io, MPL-2.0, by Vicki Pfau and contributors) exposes its
 * emulator through `struct mCore`, a table of function pointers. Binding that
 * struct directly from Rust would be fragile (its layout depends on build
 * flags), so this file wraps just what GBADE needs behind plain functions.
 */
#include <mgba/flags.h>

#include <mgba/core/blip_buf.h>
#include <mgba/core/core.h>
#include <mgba/core/log.h>
#include <mgba/core/serialize.h>
#include <mgba/debugger/debugger.h>
#include <mgba/internal/arm/arm.h>
#include <mgba/internal/arm/decoder.h>
#include <mgba-util/vfs.h>

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gbade_shim.h"

#define GBA_WIDTH 240
#define GBA_HEIGHT 160

struct GbadeEmu {
    struct mCore *core;
    color_t *video;
    uint8_t *rom_copy;
    size_t rom_size;
    struct mDebugger *debugger;
    int stop_reason;
    uint32_t stop_address;
    int breakpoint_count;
    gbade_log_fn log_fn;
    void *log_ctx;
    uint32_t audio_rate;
};

/* ------------------------------------------------------------------------- */
/* Logging                                                                    */
/* ------------------------------------------------------------------------- */
/* mGBA's logger is global; we route messages to the emulator currently running
 * on this thread. */

static _Thread_local struct GbadeEmu *current_emu;
static struct mLogger shim_logger;
static int logger_installed;

static void shim_log(struct mLogger *logger, int category, enum mLogLevel level, const char *format, va_list args) {
    (void)logger;
    struct GbadeEmu *emu = current_emu;
    if (!emu || !emu->log_fn) return;
    const char *cat = mLogCategoryId(category);
    /* Game debug prints (category "gba.debug") are always forwarded; mGBA's
     * own debug chatter is not. */
    if (level == mLOG_DEBUG && (!cat || strcmp(cat, "gba.debug") != 0)) return;
    char message[512];
    vsnprintf(message, sizeof(message), format, args);
    emu->log_fn(emu->log_ctx, (int)level, cat ? cat : "", message);
}

static void install_logger(void) {
    if (logger_installed) return;
    memset(&shim_logger, 0, sizeof(shim_logger));
    shim_logger.log = shim_log;
    shim_logger.filter = NULL;
    mLogSetDefaultLogger(&shim_logger);
    logger_installed = 1;
}

/* ------------------------------------------------------------------------- */
/* Debugger                                                                   */
/* ------------------------------------------------------------------------- */

static struct GbadeEmu *emu_for_debugger(struct mDebugger *d) {
    /* We store the owning emu right after the mDebugger struct. */
    return *(struct GbadeEmu **)((char *)d + sizeof(struct mDebugger));
}

static void shim_debugger_entered(struct mDebugger *d, enum mDebuggerEntryReason reason, struct mDebuggerEntryInfo *info) {
    struct GbadeEmu *emu = emu_for_debugger(d);
    switch (reason) {
    case DEBUGGER_ENTER_BREAKPOINT:
        emu->stop_reason = GBADE_STOP_BREAKPOINT;
        break;
    case DEBUGGER_ENTER_ILLEGAL_OP:
        emu->stop_reason = GBADE_STOP_ILLEGAL_OP;
        break;
    case DEBUGGER_ENTER_WATCHPOINT:
        emu->stop_reason = GBADE_STOP_WATCHPOINT;
        break;
    default:
        emu->stop_reason = GBADE_STOP_OTHER;
        break;
    }
    emu->stop_address = info ? info->address : 0;
}

static void attach_debugger(struct GbadeEmu *emu) {
    if (emu->debugger || !emu->core) return;
    void *mem = calloc(1, sizeof(struct mDebugger) + sizeof(struct GbadeEmu *));
    struct mDebugger *d = mem;
    *(struct GbadeEmu **)((char *)mem + sizeof(struct mDebugger)) = emu;
    d->type = DEBUGGER_CUSTOM;
    d->entered = shim_debugger_entered;
    d->state = DEBUGGER_RUNNING;
    mDebuggerAttach(d, emu->core);
    emu->debugger = d;
}

/* ------------------------------------------------------------------------- */
/* Lifecycle                                                                  */
/* ------------------------------------------------------------------------- */

struct GbadeEmu *gbade_emu_create(void) {
    install_logger();
    struct GbadeEmu *emu = calloc(1, sizeof(*emu));
    emu->video = calloc(GBA_WIDTH * GBA_HEIGHT, sizeof(color_t));
    emu->audio_rate = 48000;
    return emu;
}

static void unload(struct GbadeEmu *emu) {
    if (emu->core) {
        if (emu->debugger) {
            emu->core->detachDebugger(emu->core);
            free(emu->debugger);
            emu->debugger = NULL;
        }
        emu->core->deinit(emu->core);
        emu->core = NULL;
    }
    free(emu->rom_copy);
    emu->rom_copy = NULL;
    emu->breakpoint_count = 0;
}

void gbade_emu_destroy(struct GbadeEmu *emu) {
    if (!emu) return;
    current_emu = emu;
    unload(emu);
    current_emu = NULL;
    free(emu->video);
    free(emu);
}

void gbade_emu_set_log_callback(struct GbadeEmu *emu, gbade_log_fn fn, void *ctx) {
    emu->log_fn = fn;
    emu->log_ctx = ctx;
}

static void setup_audio(struct GbadeEmu *emu) {
    struct mCore *core = emu->core;
    core->setAudioBufferSize(core, 2048);
    blip_set_rates(core->getAudioChannel(core, 0), core->frequency(core), emu->audio_rate);
    blip_set_rates(core->getAudioChannel(core, 1), core->frequency(core), emu->audio_rate);
}

int gbade_emu_load_rom(struct GbadeEmu *emu, const uint8_t *rom, size_t rom_len, const uint8_t *save, size_t save_len) {
    current_emu = emu;
    unload(emu);

    emu->rom_copy = malloc(rom_len);
    memcpy(emu->rom_copy, rom, rom_len);
    emu->rom_size = rom_len;
    struct VFile *vf = VFileFromMemory(emu->rom_copy, rom_len);
    if (!vf) return 0;

    struct mCore *core = mCoreFindVF(vf);
    if (!core) {
        vf->close(vf);
        return 0;
    }
    mCoreInitConfig(core, NULL);
    if (!core->init(core)) {
        vf->close(vf);
        return 0;
    }
    emu->core = core;
    core->setVideoBuffer(core, emu->video, GBA_WIDTH);
    setup_audio(emu);

    if (!core->loadROM(core, vf)) {
        unload(emu);
        return 0;
    }

    /* Save data lives in memory; GBADE reads it back with gbade_emu_savedata
     * and writes it to disk itself. */
    struct VFile *sav = VFileMemChunk(save, save ? save_len : 0);
    core->loadSave(core, sav);

    attach_debugger(emu);
    core->reset(core);
    emu->stop_reason = GBADE_STOP_NONE;
    current_emu = NULL;
    return 1;
}

void gbade_emu_reset(struct GbadeEmu *emu) {
    if (!emu->core) return;
    current_emu = emu;
    emu->core->reset(emu->core);
    emu->stop_reason = GBADE_STOP_NONE;
    if (emu->debugger) emu->debugger->state = DEBUGGER_RUNNING;
    current_emu = NULL;
}

/* ------------------------------------------------------------------------- */
/* Running                                                                    */
/* ------------------------------------------------------------------------- */

int gbade_emu_run_frame(struct GbadeEmu *emu) {
    if (!emu->core) return GBADE_STOP_OTHER;
    current_emu = emu;
    struct mCore *core = emu->core;
    struct mDebugger *d = emu->debugger;

    if (d && d->state == DEBUGGER_PAUSED) {
        /* Resuming after a stop. */
        d->state = DEBUGGER_RUNNING;
    }
    emu->stop_reason = GBADE_STOP_NONE;

    if (!d || !d->platform->hasBreakpoints(d->platform)) {
        core->runFrame(core);
    } else {
        uint32_t frame = core->frameCounter(core);
        while (core->frameCounter(core) == frame) {
            mDebuggerRun(d);
            if (d->state == DEBUGGER_PAUSED) break;
        }
    }
    if (d && d->state == DEBUGGER_PAUSED && emu->stop_reason == GBADE_STOP_NONE) {
        emu->stop_reason = GBADE_STOP_OTHER;
    }
    current_emu = NULL;
    return emu->stop_reason;
}

int gbade_emu_step(struct GbadeEmu *emu) {
    if (!emu->core) return GBADE_STOP_OTHER;
    current_emu = emu;
    if (emu->debugger) emu->debugger->state = DEBUGGER_RUNNING;
    emu->stop_reason = GBADE_STOP_NONE;
    emu->core->step(emu->core);
    current_emu = NULL;
    return GBADE_STOP_NONE;
}

uint32_t gbade_emu_stop_address(struct GbadeEmu *emu) { return emu->stop_address; }
uint32_t gbade_emu_frame_counter(struct GbadeEmu *emu) {
    return emu->core ? emu->core->frameCounter(emu->core) : 0;
}

const uint32_t *gbade_emu_framebuffer(struct GbadeEmu *emu) { return (const uint32_t *)emu->video; }

void gbade_emu_set_keys(struct GbadeEmu *emu, uint32_t keys) {
    if (emu->core) emu->core->setKeys(emu->core, keys);
}

/* ------------------------------------------------------------------------- */
/* Audio                                                                      */
/* ------------------------------------------------------------------------- */

void gbade_emu_set_audio_rate(struct GbadeEmu *emu, uint32_t rate) {
    emu->audio_rate = rate ? rate : 48000;
    if (emu->core) setup_audio(emu);
}

size_t gbade_emu_read_audio(struct GbadeEmu *emu, int16_t *out, size_t max_frames) {
    if (!emu->core) return 0;
    blip_t *left = emu->core->getAudioChannel(emu->core, 0);
    blip_t *right = emu->core->getAudioChannel(emu->core, 1);
    int avail = blip_samples_avail(left);
    if ((size_t)avail > max_frames) avail = (int)max_frames;
    if (avail <= 0) return 0;
    int got = blip_read_samples(left, out, avail, 1);
    blip_read_samples(right, out + 1, avail, 1);
    return (size_t)got;
}

/* ------------------------------------------------------------------------- */
/* Memory & CPU inspection                                                    */
/* ------------------------------------------------------------------------- */

uint32_t gbade_emu_read(struct GbadeEmu *emu, uint32_t address, int width) {
    if (!emu->core) return 0;
    switch (width) {
    case 1: return emu->core->rawRead8(emu->core, address, -1);
    case 2: return emu->core->rawRead16(emu->core, address, -1);
    default: return emu->core->rawRead32(emu->core, address, -1);
    }
}

void gbade_emu_read_block(struct GbadeEmu *emu, uint32_t address, uint8_t *out, size_t len) {
    for (size_t i = 0; i < len; i++) {
        out[i] = emu->core ? (uint8_t)emu->core->rawRead8(emu->core, address + (uint32_t)i, -1) : 0;
    }
}

void gbade_emu_write(struct GbadeEmu *emu, uint32_t address, uint32_t value, int width) {
    if (!emu->core) return;
    switch (width) {
    case 1: emu->core->rawWrite8(emu->core, address, -1, (uint8_t)value); break;
    case 2: emu->core->rawWrite16(emu->core, address, -1, (uint16_t)value); break;
    default: emu->core->rawWrite32(emu->core, address, -1, value); break;
    }
}

void gbade_emu_cpu_state(struct GbadeEmu *emu, uint32_t regs[16], uint32_t *cpsr) {
    if (!emu->core) return;
    struct ARMCore *cpu = emu->core->cpu;
    for (int i = 0; i < 16; i++) regs[i] = (uint32_t)cpu->gprs[i];
    /* gprs[PC] points 2 instructions ahead (pipeline); report the current one. */
    int thumb = cpu->executionMode == MODE_THUMB;
    regs[15] -= thumb ? WORD_SIZE_THUMB : WORD_SIZE_ARM;
    *cpsr = cpu->cpsr.packed;
}

/* Disassemble one instruction at `address` into `out`. Returns its size. */
int gbade_emu_disassemble(struct GbadeEmu *emu, uint32_t address, int thumb, char *out, size_t out_len) {
    if (!emu->core || out_len == 0) return 0;
    struct ARMInstructionInfo info;
    if (thumb) {
        uint16_t op = (uint16_t)emu->core->rawRead16(emu->core, address, -1);
        ARMDecodeThumb(op, &info);
        /* 32-bit BL is two Thumb halves; combine them like the CPU does. */
        if (info.branchType == ARM_BRANCH_LINKED && (op & 0xF800) == 0xF000) {
            struct ARMInstructionInfo info2;
            struct ARMInstructionInfo combined;
            uint16_t op2 = (uint16_t)emu->core->rawRead16(emu->core, address + 2, -1);
            ARMDecodeThumb(op2, &info2);
            if (ARMDecodeThumbCombine(&info, &info2, &combined)) {
                ARMDisassemble(&combined, emu->core->cpu, emu->core->symbolTable, address + 4, out, (int)out_len);
                return 4;
            }
        }
        ARMDisassemble(&info, emu->core->cpu, emu->core->symbolTable, address + 4, out, (int)out_len);
        return 2;
    }
    uint32_t op = emu->core->rawRead32(emu->core, address, -1);
    ARMDecodeARM(op, &info);
    ARMDisassemble(&info, emu->core->cpu, emu->core->symbolTable, address + 8, out, (int)out_len);
    return 4;
}

/* ------------------------------------------------------------------------- */
/* Breakpoints                                                                */
/* ------------------------------------------------------------------------- */

int gbade_emu_add_breakpoint(struct GbadeEmu *emu, uint32_t address) {
    if (!emu->debugger) return -1;
    struct mBreakpoint bp = {
        .address = address,
        .segment = -1,
        .type = BREAKPOINT_HARDWARE,
    };
    ssize_t id = emu->debugger->platform->setBreakpoint(emu->debugger->platform, &bp);
    if (id >= 0) emu->breakpoint_count++;
    return (int)id;
}

void gbade_emu_clear_breakpoints(struct GbadeEmu *emu) {
    if (!emu->debugger) return;
    struct mBreakpointList list;
    mBreakpointListInit(&list, 0);
    emu->debugger->platform->listBreakpoints(emu->debugger->platform, &list);
    for (size_t i = 0; i < mBreakpointListSize(&list); i++) {
        emu->debugger->platform->clearBreakpoint(emu->debugger->platform, mBreakpointListGetPointer(&list, i)->id);
    }
    mBreakpointListDeinit(&list);
    emu->breakpoint_count = 0;
}

/* ------------------------------------------------------------------------- */
/* Save states & save data                                                    */
/* ------------------------------------------------------------------------- */

size_t gbade_emu_state_size(struct GbadeEmu *emu) {
    return emu->core ? emu->core->stateSize(emu->core) : 0;
}

int gbade_emu_save_state(struct GbadeEmu *emu, uint8_t *out, size_t len) {
    if (!emu->core || len < emu->core->stateSize(emu->core)) return 0;
    return emu->core->saveState(emu->core, out) ? 1 : 0;
}

int gbade_emu_load_state(struct GbadeEmu *emu, const uint8_t *in, size_t len) {
    if (!emu->core || len < emu->core->stateSize(emu->core)) return 0;
    current_emu = emu;
    int ok = emu->core->loadState(emu->core, in) ? 1 : 0;
    current_emu = NULL;
    return ok;
}

size_t gbade_emu_savedata(struct GbadeEmu *emu, uint8_t *out, size_t max_len) {
    if (!emu->core) return 0;
    void *sram = NULL;
    size_t size = emu->core->savedataClone(emu->core, &sram);
    if (!sram) return 0;
    if (out && max_len) memcpy(out, sram, size < max_len ? size : max_len);
    free(sram);
    return size;
}
