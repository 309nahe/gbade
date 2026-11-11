@ =============================================================================
@  64-bit divide-with-remainder helpers required by the ARM EABI.
@ =============================================================================
@
@  The ARM7TDMI has no divide instruction at all. When you write `a / b` or
@  `a % b` on 64-bit integers (long long), the compiler emits a call to
@  __aeabi_ldivmod / __aeabi_uldivmod. These must return the quotient in r0:r1
@  and the remainder in r2:r3, which cannot be expressed in plain C, so they
@  are tiny assembly wrappers around compiler-rt's __divmoddi4/__udivmoddi4.
@
@  They are written in Thumb so that they can be called from Thumb code (which
@  is what GBADE compiles your game to) without any interworking veneer.
@ =============================================================================

    .syntax unified
    .text
    .thumb
    .align 2

    .global __aeabi_uldivmod
    .type   __aeabi_uldivmod, %function
    .thumb_func
__aeabi_uldivmod:
    push    {r4, lr}
    sub     sp, sp, #16             @ [sp] = 5th arg, [sp+8] = remainder
    add     r4, sp, #8
    str     r4, [sp]
    bl      __udivmoddi4            @ r0:r1 = quotient, remainder at [sp+8]
    ldr     r2, [sp, #8]
    ldr     r3, [sp, #12]
    add     sp, sp, #16
    ldr     r4, [sp, #4]            @ saved lr
    mov     lr, r4
    pop     {r4}
    add     sp, sp, #4
    bx      lr
    .size   __aeabi_uldivmod, . - __aeabi_uldivmod

    .global __aeabi_ldivmod
    .type   __aeabi_ldivmod, %function
    .thumb_func
__aeabi_ldivmod:
    push    {r4, lr}
    sub     sp, sp, #16
    add     r4, sp, #8
    str     r4, [sp]
    bl      __divmoddi4
    ldr     r2, [sp, #8]
    ldr     r3, [sp, #12]
    add     sp, sp, #16
    ldr     r4, [sp, #4]
    mov     lr, r4
    pop     {r4}
    add     sp, sp, #4
    bx      lr
    .size   __aeabi_ldivmod, . - __aeabi_ldivmod
