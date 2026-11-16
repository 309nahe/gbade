# 4. Smooth Movement

Whole pixels per frame are too coarse: 1 is slow, 2 is already fast, and
gravity needs something in between. The answer on the GBA is **fixed-point**
numbers.

## What you will learn

- Why GBA games avoid `float`.
- How a fixed-point number stores fractions in an ordinary integer.
- Velocity and gravity: the two lines behind every jump.

## No floating point unit

The GBA's ARM7TDMI processor has no hardware for floating-point maths.
`float` still compiles, but every operation is a slow software routine. Tonc
puts it simply: on the GBA it is fixed-point math all the way.

## Counting in fractions

Tonc's comparison: $10.78 is awkward as an integer, but 1078 *cents* is not.
Fixed-point does the same with binary fractions. GBADE's `fixed` type is a
32-bit integer whose low 8 bits are the fraction (the "24.8" format Tonc also
uses): 1.0 is stored as 256, 0.5 as 128.

```c
fixed x = INT_TO_FIX(10);   // 10.0, stored as 2560
fixed speed = FIX(1.5);     // 1.5, stored as 384 (constants only)
x += speed;                 // adding works as usual
int pixel = FIX_TO_INT(x);  // drop the fraction to draw: 11
fixed half = fx_mul(speed, FIX(0.5));  // multiplying needs fx_mul
```

Addition and subtraction work directly. Multiplication and division need
`fx_mul` and `fx_div`, which correct the extra factor of 256.

## Velocity and gravity

Keep a position and a velocity, both fixed-point. Every frame:

```c
vy += GRAVITY;   // gravity changes the velocity a little
y  += vy;        // the velocity changes the position
```

A jump is just setting `vy` to a negative number once. Bouncing is flipping
the velocity's sign and keeping a part of it (`fx_mul(vy, FIX(0.7))`).

## Run the example

Click **▶ Try this example**. A ball falls and bounces. Press **X** (A) to
throw it up again, and the arrows to push it sideways. Each throw prints the
height with its fraction in the Console.

## Play with it

- Change `GRAVITY`: try `FIX(0.05)` (the Moon) and `FIX(0.5)`.
- Make the bounce lose less or more energy.
- Add friction on the ground: multiply `vx` by `FIX(0.9)` every frame.

## Go further

- [Tonc: Fixed-point math & LUTs](https://gbadev.net/tonc/fixed.html) covers
  the formats, rounding, division and lookup tables.
- [Tonc: Numbers, bits and bit operations](https://gbadev.net/tonc/numbers.html)
  is the background for shifts and masks.
- GBADE's `gba/math.h` also has `lu_sin` and `lu_cos` for circles and angles.
