/*
 * GBADE runtime: small ARM EABI helpers that compiler-rt only provides in
 * assembly for newer ARM cores. Written in C so they work on the ARM7TDMI.
 *
 * You never call these yourself: the compiler calls them for operations the
 * GBA CPU cannot do in hardware (division, floating point comparisons...).
 */

typedef unsigned int u32;
typedef unsigned long long u64;

int __divsi3(int a, int b);
unsigned __udivsi3(unsigned a, unsigned b);

/* Returns quotient in r0 and remainder in r1 (packed in a 64-bit value). */
u64 __aeabi_idivmod(int a, int b) {
    int q = __divsi3(a, b);
    int r = a - q * b;
    return (u64)(u32)q | ((u64)(u32)r << 32);
}

u64 __aeabi_uidivmod(unsigned a, unsigned b) {
    unsigned q = __udivsi3(a, b);
    unsigned r = a - q * b;
    return (u64)q | ((u64)r << 32);
}

/* Division by zero hooks: return 0 like most GBA toolchains. */
int __aeabi_idiv0(int r) { (void)r; return 0; }
long long __aeabi_ldiv0(long long r) { (void)r; return 0; }

/* Single precision comparisons (soft-float ABI: floats travel in r0/r1). */
int __eqsf2(float, float);
int __ltsf2(float, float);
int __lesf2(float, float);
int __gesf2(float, float);
int __gtsf2(float, float);
int __aeabi_fcmpeq(float a, float b) { return __eqsf2(a, b) == 0; }
int __aeabi_fcmplt(float a, float b) { return __ltsf2(a, b) < 0; }
int __aeabi_fcmple(float a, float b) { return __lesf2(a, b) <= 0; }
int __aeabi_fcmpge(float a, float b) { return __gesf2(a, b) >= 0; }
int __aeabi_fcmpgt(float a, float b) { return __gtsf2(a, b) > 0; }

/* Double precision comparisons. */
int __eqdf2(double, double);
int __ltdf2(double, double);
int __ledf2(double, double);
int __gedf2(double, double);
int __gtdf2(double, double);
int __aeabi_dcmpeq(double a, double b) { return __eqdf2(a, b) == 0; }
int __aeabi_dcmplt(double a, double b) { return __ltdf2(a, b) < 0; }
int __aeabi_dcmple(double a, double b) { return __ledf2(a, b) <= 0; }
int __aeabi_dcmpge(double a, double b) { return __gedf2(a, b) >= 0; }
int __aeabi_dcmpgt(double a, double b) { return __gtdf2(a, b) > 0; }

/* compiler-rt calls this on "impossible" situations. Halt the CPU. */
void __compilerrt_abort_impl(const char *file, int line, const char *function) {
    (void)file; (void)line; (void)function;
    for (;;) {
    }
}
