# Saving the Game

Most GBA cartridges carry a small battery-backed memory. GBADE supports the
simplest kind, SRAM: 32 KB at `0x0E000000` that your game can read and write
like ordinary memory. A high score, a completed level list or a settings
block fits easily.

## Reading and writing

```c
#include <gba.h>

typedef struct {
    u32 magic;
    int best_score;
    u8  levels_done;
} SaveData;

SaveData s;

save_read(0, &s, sizeof s);
if (s.magic != 0x47424144) {        /* "GBAD" */
    s.magic = 0x47424144;
    s.best_score = 0;
    s.levels_done = 0;
    save_write(0, &s, sizeof s);
}

/* Later, when something changes: */
if (score > s.best_score) {
    s.best_score = score;
    save_write(0, &s, sizeof s);
}
```

- `save_read(offset, data, size)` copies from SRAM into your struct.
- `save_write(offset, data, size)` copies your struct into SRAM.
- `save_erase()` fills the whole 32 KB with `0xFF`, the value a blank
  cartridge has.

Offsets and sizes are clamped to 32 KB, so a wrong size cannot corrupt memory
outside the save area.

## Check the magic number

SRAM on a new cartridge, or after an erase, is all `0xFF`. Reading that into a
struct gives nonsense values, which is why the first field should be a magic
number. If it does not match, treat the save as empty and write a fresh one.

Never assume a save exists. A player may delete it, play a ROM with no save
hardware, or emulate a cartridge type without a battery.

## Writing is slow, so write rarely

SRAM is an 8-bit bus and each `save_write` is a byte loop. Writing a whole
struct every frame would eat a visible slice of the frame time for no reason.
(Some cartridges use flash memory instead of SRAM, and flash does wear out, so
writing rarely is a good habit anyway.) Write when something actually changes:
a new high score, the end of a level, leaving the options screen.

If a structure changes size between versions, old saves will not match the new
layout. Keep a version field next to the magic number and migrate or reset
when it differs:

```c
#define SAVE_VERSION 2
typedef struct { u32 magic; u16 version; u16 padding; /* ... */ } SaveData;
```

## Detecting the save type

Emulators and flashcarts decide which save chip a ROM expects by looking for a
marker string in the ROM. The SDK embeds `SRAM_V113` as soon as your program
links a save function, and mGBA finds it. You do not need to configure
anything.

## Where GBADE keeps the save

The emulator writes the cartridge's save data to:

```text
<project>/build/<game-name>.sav
```

`gbade run` loads that file before starting and writes it back at the end, so
a headless run sees the same high score as the IDE. Deleting the `.sav` file
resets the game's save, which is the fastest way to test a first-run screen.

Exporting the ROM copies the ROM only. To keep a save on real hardware, use
the same `.sav` file with your flashcart or emulator.

## Save states are different

**F2** and **F4** in the IDE save and load the *whole machine*: CPU, memory,
video and save RAM. A save state is for debugging and for trying a risky
change; it lives in the IDE, not in the game, and it is not a substitute for
the game's own save file.

## Try it

The [Pong example](../../examples/pong/README.md) keeps its best rally, the high score, in SRAM.
Play a round, close the game, run it again: the score is still there because
`build/pong.sav` survived.
