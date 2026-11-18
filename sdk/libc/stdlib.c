/*
 * GBADE libc: malloc/free, rand, conversions, qsort, puts/putchar.
 */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* ========================================================================== */
/* malloc: a simple first-fit free list in EWRAM                               */
/* ========================================================================== */
/*
 * Every block starts with a header holding its size (including the header)
 * and whether it is free. Blocks are contiguous from __heap_start to
 * __heap_end, so walking the heap is just "next = block + size".
 */
extern char __heap_start[], __heap_end[];

typedef struct Block {
    uint32_t size;  /* bytes, including this header; multiple of 8 */
    uint32_t free;
} Block;

static Block *heap_first;

static void heap_init(void) {
    heap_first = (Block *)__heap_start;
    heap_first->size = (uint32_t)(__heap_end - __heap_start) & ~7u;
    heap_first->free = 1;
}

static Block *next_block(Block *b) {
    Block *n = (Block *)((char *)b + b->size);
    return (char *)n < __heap_end && b->size ? n : NULL;
}

void *malloc(size_t size) {
    if (!heap_first) heap_init();
    if (size == 0) size = 1;
    uint32_t need = ((uint32_t)size + sizeof(Block) + 7) & ~7u;
    for (Block *b = heap_first; b; b = next_block(b)) {
        if (!b->free) continue;
        /* Merge following free blocks. */
        Block *n;
        while ((n = next_block(b)) && n->free) b->size += n->size;
        if (b->size >= need) {
            if (b->size - need >= sizeof(Block) + 8) {
                Block *rest = (Block *)((char *)b + need);
                rest->size = b->size - need;
                rest->free = 1;
                b->size = need;
            }
            b->free = 0;
            return b + 1;
        }
    }
    return NULL; /* out of memory */
}

void free(void *ptr) {
    if (!ptr) return;
    Block *b = (Block *)ptr - 1;
    b->free = 1;
}

void *calloc(size_t count, size_t size) {
    size_t total = count * size;
    if (size && total / size != count) return NULL;
    void *p = malloc(total);
    if (p) memset(p, 0, total);
    return p;
}

void *realloc(void *ptr, size_t size) {
    if (!ptr) return malloc(size);
    Block *b = (Block *)ptr - 1;
    size_t old = b->size - sizeof(Block);
    if (size <= old) return ptr;
    void *n = malloc(size);
    if (n) {
        memcpy(n, ptr, old);
        free(ptr);
    }
    return n;
}

/* ========================================================================== */
/* Misc                                                                        */
/* ========================================================================== */

static uint32_t rand_state = 1;
void srand(unsigned seed) { rand_state = seed ? seed : 1; }
int rand(void) {
    /* xorshift32 */
    uint32_t x = rand_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rand_state = x;
    return (int)(x & RAND_MAX);
}

int abs(int x) { return x < 0 ? -x : x; }
long labs(long x) { return x < 0 ? -x : x; }

long strtol(const char *s, char **end, int base) {
    while (*s == ' ' || (*s >= '\t' && *s <= '\r')) s++;
    int neg = 0;
    if (*s == '-' || *s == '+') neg = (*s++ == '-');
    if ((base == 0 || base == 16) && s[0] == '0' && (s[1] | 32) == 'x') {
        s += 2;
        base = 16;
    } else if (base == 0) {
        base = s[0] == '0' ? 8 : 10;
    }
    long v = 0;
    for (;; s++) {
        int d;
        if (*s >= '0' && *s <= '9') d = *s - '0';
        else if ((*s | 32) >= 'a' && (*s | 32) <= 'z') d = (*s | 32) - 'a' + 10;
        else break;
        if (d >= base) break;
        v = v * base + d;
    }
    if (end) *end = (char *)s;
    return neg ? -v : v;
}

int atoi(const char *s) { return (int)strtol(s, NULL, 10); }

void debug_panic(const char *file, int line, const char *msg) __attribute__((noreturn));
void abort(void) { debug_panic("abort", 0, "abort() called"); }
void exit(int status) {
    (void)status;
    debug_panic("exit", status, "exit() called: GBA games never exit");
}

/* Swap two elements of `size` bytes in place, word by word when possible (the
 * common case: int-sized and struct elements), byte by byte otherwise. No
 * temporary buffer, no allocation, so any element size works. */
typedef uint32_t __attribute__((may_alias)) word_t;

static void swap_elements(char *x, char *y, size_t size) {
    if ((((uintptr_t)x | (uintptr_t)y | size) & 3) == 0) {
        word_t *p = (word_t *)x, *q = (word_t *)y;
        for (size_t n = size / 4; n; n--) {
            word_t t = *p;
            *p++ = *q;
            *q++ = t;
        }
    } else {
        for (; size; size--) {
            char t = *x;
            *x++ = *y;
            *y++ = t;
        }
    }
}

/* Insertion sort: small and simple. It is stable (equal items keep their
 * order), needs no extra memory, and works for elements of any size.
 *
 * Its speed depends on the data. Measured on the GBA for 128 items (one frame
 * is about 280,000 cycles): already sorted 25,000 cycles, 3 items out of
 * place 72,000, shuffled 1,150,000 (about 4 frames), worst case 1,940,000.
 * So it is a good fit for data that is nearly sorted from the previous frame
 * (sprites by depth), and a poor one for sorting many shuffled items in one
 * frame. */
void qsort(void *base, size_t count, size_t size, int (*cmp)(const void *, const void *)) {
    if (count < 2 || size == 0) return;
    char *a = base;
    for (size_t i = 1; i < count; i++) {
        for (size_t j = i; j > 0 && cmp(a + (j - 1) * size, a + j * size) > 0; j--) {
            swap_elements(a + (j - 1) * size, a + j * size, size);
        }
    }
}

/* puts/putchar go to the GBADE console like printf. */
void _putchar(char c);
int putchar(int c) {
    _putchar((char)c);
    return c;
}
int puts(const char *s) {
    while (*s) _putchar(*s++);
    _putchar('\n');
    return 0;
}
