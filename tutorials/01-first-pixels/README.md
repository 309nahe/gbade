# 1. First Pixels

Put colours on the screen by writing numbers into memory. This is the
"hello world" of GBA graphics, and it is how the community's best-known
tutorial, Tonc, starts too.

## What you will learn

- The GBA screen is 240 by 160 pixels.
- In **mode 3**, video memory is one big picture: one 16-bit number per pixel.
- A colour is 5 bits of red, 5 of green and 5 of blue.

## The screen is an array

The display control register, `REG_DISPCNT`, says what the screen shows.
Setting it to mode 3 with background 2 turns video memory (VRAM, at address
`0x06000000`) into a 240x160 bitmap, like an image file. Pixel (x, y) is
element `y * 240 + x` of that array.

```c
REG_DISPCNT = DCNT_MODE3 | DCNT_BG2;   // bitmap mode, show background 2
m3_mem[80 * 240 + 120] = CLR_RED;      // the pixel at x=120, y=80
```

`m3_plot(x, y, colour)` does the same thing and ignores pixels outside the
screen, so a mistake cannot write into the wrong memory.

## Colours are 15 bits

Each pixel holds red, green and blue from 0 to 31: `RGB15(31, 0, 0)` is pure
red, `RGB15(31, 31, 31)` is white. That makes 32,768 possible colours.
`RGB8(r, g, b)` accepts the 0-255 values you know from paint programs and
converts them. Ready-made names exist too: `CLR_RED`, `CLR_SKY`...

## Drawing helpers

Writing pixels one by one gets tedious, so the GBADE SDK has small helpers:
`m3_fill`, `m3_rect`, `m3_line`, `m3_circle` and `m3_text`. They are plain C
loops over the same array; nothing magic.

## Run the example

Click **▶ Try this example** at the top of the page. GBADE copies the example
into a new project and runs it. You will see three pixels, a few shapes and
a line of text.

## Play with it

- Change the background colour in `m3_fill`.
- Draw a smiley face with `m3_circle` and `m3_line`.
- Make a gradient: a loop that draws 160 lines, each a slightly different
  colour (`RGB15(0, 0, y * 31 / 159)`).

## Go further

- [Tonc: My First GBA Demo](https://gbadev.net/tonc/first.html) explains
  every line of the classic three-pixel program.
- [Tonc: The bitmap modes](https://gbadev.net/tonc/bitmaps.html) covers modes
  3, 4 and 5, and why games rarely use them for everything.
- The **Graphics** guide in GBADE's Learn panel lists the drawing functions.
