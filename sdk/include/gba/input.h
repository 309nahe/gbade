/*
 * gba/input.h - buttons
 *
 * The GBA has 10 buttons: A, B, Select, Start, the D-pad (4 directions), L, R.
 * Their state is in REG_KEYINPUT, one bit per button. Careful: a bit is 0 when
 * the button is PRESSED and 1 when released ("active low").
 *
 * The easiest way is to call key_poll() once per frame and then ask:
 *   key_held(KEY_A)      is A down right now?
 *   key_hit(KEY_A)       was A pressed THIS frame (it was up last frame)?
 *   key_released(KEY_A)  was A released this frame?
 *
 * In GBADE's emulator the default keyboard mapping is:
 *   arrows = D-pad, X = A, Z = B, Enter = Start, Backspace = Select,
 *   A = L, S = R   (configurable in the app settings)
 */
#ifndef GBA_INPUT_H
#define GBA_INPUT_H

#include "types.h"

#define REG_KEYINPUT    (*(vu16 *)0x04000130)
#define REG_KEYCNT      (*(vu16 *)0x04000132)

#define KEY_A           0x0001
#define KEY_B           0x0002
#define KEY_SELECT      0x0004
#define KEY_START       0x0008
#define KEY_RIGHT       0x0010
#define KEY_LEFT        0x0020
#define KEY_UP          0x0040
#define KEY_DOWN        0x0080
#define KEY_R           0x0100
#define KEY_L           0x0200
#define KEY_ANY         0x03FF
#define KEY_DPAD        0x00F0

/* Read the buttons. Call once per frame, before the key_* functions. */
void key_poll(void);

/* Buttons currently held down (bit mask of KEY_*). */
u16 key_held_mask(void);

bool key_held(u16 keys);      /* any of `keys` is down */
bool key_hit(u16 keys);       /* any of `keys` went down this frame */
bool key_released(u16 keys);  /* any of `keys` went up this frame */

/* -1, 0 or +1 depending on the D-pad. Handy for movement:
 *     x += key_tri_horz() * speed; */
int key_tri_horz(void);   /* left = -1, right = +1 */
int key_tri_vert(void);   /* up = -1, down = +1 */

#endif /* GBA_INPUT_H */
