/*
 * gba/save.h - saving the game
 *
 * Most GBA cartridges have a small battery-backed memory for save games. The
 * simplest kind is SRAM: 32 KB at 0x0E000000, readable and writable like
 * normal memory, but ONLY ONE BYTE AT A TIME (8-bit bus).
 *
 * Using any function below automatically marks your ROM as "has SRAM" so
 * emulators (and GBADE) know to keep the save. GBADE stores it next to your
 * project in build/<name>.sav.
 *
 *     typedef struct { u32 magic; int best_score; } SaveData;
 *     SaveData s;
 *     save_read(0, &s, sizeof s);
 *     if (s.magic != 0x47424144) { s.magic = 0x47424144; s.best_score = 0; }
 *     ...
 *     save_write(0, &s, sizeof s);
 */
#ifndef GBA_SAVE_H
#define GBA_SAVE_H

#include "types.h"

#define SAVE_SIZE 0x8000

void save_write(u32 offset, const void *data, u32 size);
void save_read(u32 offset, void *data, u32 size);
void save_erase(void);   /* fill with 0xFF (the "empty" value) */

#endif /* GBA_SAVE_H */
