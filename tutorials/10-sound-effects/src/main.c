// Tutorial 10: Sound Effects
//
// A (X) jump, B (Z) coin, L (A) explosion, R (S) melody.

#include <gba.h>
#include <stdio.h>

// A melody is a list of (note, length in frames). 0 is a rest.
typedef struct {
    int hz;
    int frames;
} Note;

static const Note melody[] = {
    { NOTE_C4, 10 }, { NOTE_E4, 10 }, { NOTE_G4, 10 }, { NOTE_C5, 20 },
    { 0, 6 },        { NOTE_G4, 10 }, { NOTE_C5, 30 },
};

int main(void) {
    text_init();
    text_print_at(6, 3, "SOUND EFFECTS");
    text_print_at(4, 7, "A (X key)  jump");
    text_print_at(4, 9, "B (Z key)  coin");
    text_print_at(4, 11, "L (A key)  explosion");
    text_print_at(4, 13, "R (S key)  melody");

    sound_init();                        // switch the sound hardware on

    int next_note = -1;                  // -1: no melody playing
    int note_timer = 0;

    while (1) {
        vsync();                         // sound lengths are counted in frames
        key_poll();

        if (key_hit(KEY_A)) {
            sound_beep(NOTE_C5, 6);      // channel 1: a short square wave
            printf("jump\n");
        }
        if (key_hit(KEY_B)) {
            sound_tone(NOTE_A4 * 2, 5, 1);   // channel 2, a thin "ding"
            printf("coin\n");
        }
        if (key_hit(KEY_L)) {
            sound_noise(24, 5);          // channel 4: noise
            printf("boom\n");
        }
        if (key_hit(KEY_R)) {
            next_note = 0;               // start the melody
            note_timer = 0;
            printf("melody\n");
        }

        // The melody player: one step per frame, never waiting.
        if (next_note >= 0 && --note_timer <= 0) {
            if (next_note < (int)ARRAY_SIZE(melody)) {
                const Note *n = &melody[next_note++];
                if (n->hz) sound_tone(n->hz, n->frames - 1, 2);
                note_timer = n->frames;
            } else {
                next_note = -1;          // finished
            }
        }
    }
}
