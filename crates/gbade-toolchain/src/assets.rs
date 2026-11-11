//! Converting PNG images in `assets/` into C arrays the GBA can use directly.
//!
//! For every `assets/<name>.png` GBADE generates, in `build/gen/assets.c` and
//! `build/gen/assets.h` (include it with `#include "assets.h"`):
//!
//! | symbol              | what                                                    |
//! |---------------------|---------------------------------------------------------|
//! | `name_pal[]`        | the palette (16 or 256 colours, BGR555). Index 0 = transparent |
//! | `name_tiles[]`      | the image cut into 8x8 tiles (4bpp if <= 15 colours, else 8bpp) |
//! | `name_bitmap[]`     | 16-bit pixels for mode 3 (only if the image fits 240x160) |
//! | `name_map[]`        | images in `assets/backgrounds/`: a tile map + de-duplicated tiles |
//! | `NAME_WIDTH` etc.   | sizes, tile count, bits per pixel                        |
//!
//! Transparency: fully transparent pixels (alpha < 128) and magenta
//! (255, 0, 255) both become colour index 0, which the GBA never draws.
//!
//! Sprite sheets: name the file `hero.16x16.png` and every 16x16 frame is
//! stored as consecutive tiles (what sprites need with 1D mapping), so frame
//! `n` starts at tile `n * HERO_FRAME_TILES`.
//!
//! Unused arrays cost nothing: the linker removes them from the ROM.

use image::RgbaImage;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// A problem with one asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetError {
    pub file: PathBuf,
    pub message: String,
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.message)
    }
}

/// Description of a converted image (also used by the IDE's asset browser).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedImage {
    /// C identifier prefix, e.g. `hero`.
    pub ident: String,
    pub width: u32,
    pub height: u32,
    /// Bits per pixel of the tiles: 4 or 8 (0 if not tileable).
    pub bpp: u32,
    pub colors: usize,
    pub palette: Vec<u16>,
    /// Tiles as packed bytes (32 or 64 bytes per tile).
    pub tiles: Vec<u8>,
    pub tile_count: usize,
    /// Size of one animation frame in pixels.
    pub frame: (u32, u32),
    /// Background map (screen entries), only for `assets/backgrounds/`.
    pub map: Option<Vec<u16>>,
    pub map_size: (u32, u32),
    /// Mode 3 bitmap, if the image fits on screen.
    pub bitmap: Option<Vec<u16>>,
}

/// Convert an RGB888 colour to the GBA's 15-bit BGR555.
pub fn rgb_to_bgr555(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16) >> 3) | (((g as u16) >> 3) << 5) | (((b as u16) >> 3) << 10)
}

/// Convert BGR555 back to RGB888 (for previews).
pub fn bgr555_to_rgb(c: u16) -> (u8, u8, u8) {
    let r = (c & 31) as u8;
    let g = ((c >> 5) & 31) as u8;
    let b = ((c >> 10) & 31) as u8;
    ((r << 3) | (r >> 2), (g << 3) | (g >> 2), (b << 3) | (b >> 2))
}

/// Turn a file stem into a valid C identifier: "Hero Walk-1" -> "hero_walk_1".
pub fn c_ident(stem: &str) -> String {
    let mut s: String =
        stem.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if s.is_empty() || s.chars().next().unwrap().is_ascii_digit() {
        s.insert(0, '_');
    }
    s
}

/// Parse "hero.16x16" -> ("hero", Some((16, 16))).
fn split_frame_suffix(stem: &str) -> (String, Option<(u32, u32)>) {
    if let Some((base, suffix)) = stem.rsplit_once('.') {
        if let Some((w, h)) = suffix.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                return (base.to_string(), Some((w, h)));
            }
        }
    }
    (stem.to_string(), None)
}

fn is_transparent(px: &image::Rgba<u8>) -> bool {
    px[3] < 128 || (px[0] == 255 && px[1] == 0 && px[2] == 255)
}

/// Convert one decoded image.
pub fn convert_image(
    img: &RgbaImage,
    stem: &str,
    is_background: bool,
) -> Result<ConvertedImage, String> {
    let (base, frame) = split_frame_suffix(stem);
    let ident = c_ident(&base);
    let (w, h) = img.dimensions();

    // Build the palette: index 0 = transparent, then colours in order of
    // first appearance (top-left to bottom-right), which keeps it stable.
    let mut palette: Vec<u16> = vec![0];
    let mut index_of: HashMap<u16, u8> = HashMap::new();
    let mut indices: Vec<u8> = Vec::with_capacity((w * h) as usize);
    let mut too_many = false;
    for px in img.pixels() {
        if is_transparent(px) {
            indices.push(0);
            continue;
        }
        let c = rgb_to_bgr555(px[0], px[1], px[2]);
        let idx = match index_of.get(&c) {
            Some(&i) => i,
            None => {
                if palette.len() >= 256 {
                    too_many = true;
                    0
                } else {
                    let i = palette.len() as u8;
                    palette.push(c);
                    index_of.insert(c, i);
                    i
                }
            }
        };
        indices.push(idx);
    }
    let colors = palette.len() - 1;

    // Mode 3 bitmap (any size up to the screen, any number of colours).
    let bitmap = (w <= 240 && h <= 160).then(|| {
        img.pixels()
            .map(|px| if is_transparent(px) { 0 } else { rgb_to_bgr555(px[0], px[1], px[2]) })
            .collect::<Vec<u16>>()
    });

    let tileable = w % 8 == 0 && h % 8 == 0 && !too_many;
    if is_background && tileable && w / 8 > 32 && (w / 8 != 64 || (h / 8) % 32 != 0) {
        return Err(format!(
            "is {w}x{h} pixels; backgrounds wider than 256 pixels must be exactly 512 pixels wide \
             and 256 or 512 pixels tall (the GBA's 64x32 / 64x64 tile map sizes)"
        ));
    }
    if is_background && !tileable {
        return Err(if too_many {
            format!("has more than 255 colours; GBA tiles can use at most 255 (+ transparent). Reduce the colours in your paint program.")
        } else {
            format!("is {w}x{h} pixels; backgrounds must be a multiple of 8 in both directions")
        });
    }
    let frame = frame.unwrap_or((8, 8));
    if tileable && (frame.0 % 8 != 0 || frame.1 % 8 != 0 || w % frame.0 != 0 || h % frame.1 != 0) {
        return Err(format!(
            "frame size {}x{} must be a multiple of 8 and divide the image size {w}x{h}",
            frame.0, frame.1
        ));
    }

    let bpp = if !tileable { 0 } else if colors <= 15 { 4 } else { 8 };
    let pal_len = if bpp == 4 { 16 } else { 256 };
    let mut pal_out = palette.clone();
    pal_out.resize(pal_len.max(pal_out.len()), 0);

    let mut tiles: Vec<u8> = Vec::new();
    let mut map = None;
    let mut map_size = (0, 0);
    if tileable {
        // Tile order: frame by frame, and inside a frame row-major 8x8 tiles.
        let mut tile_list: Vec<Vec<u8>> = Vec::new();
        for fy in (0..h).step_by(frame.1 as usize) {
            for fx in (0..w).step_by(frame.0 as usize) {
                for ty in (fy..fy + frame.1).step_by(8) {
                    for tx in (fx..fx + frame.0).step_by(8) {
                        tile_list.push(extract_tile(&indices, w, tx, ty));
                    }
                }
            }
        }
        if is_background {
            let (unique, m) = build_map(&indices, w, h);
            map_size = (w / 8, h / 8);
            map = Some(m);
            tile_list = unique;
        }
        for t in &tile_list {
            tiles.extend(pack_tile(t, bpp));
        }
    }
    let tile_count = if bpp == 0 { 0 } else { tiles.len() / (bpp as usize * 8) };

    Ok(ConvertedImage {
        ident,
        width: w,
        height: h,
        bpp,
        colors,
        palette: pal_out,
        tiles,
        tile_count,
        frame,
        map,
        map_size,
        bitmap,
    })
}

fn extract_tile(indices: &[u8], w: u32, tx: u32, ty: u32) -> Vec<u8> {
    let mut t = Vec::with_capacity(64);
    for y in ty..ty + 8 {
        for x in tx..tx + 8 {
            t.push(indices[(y * w + x) as usize]);
        }
    }
    t
}

/// 4bpp: two pixels per byte, the LEFT pixel in the LOW nibble.
/// 8bpp: one byte per pixel.
fn pack_tile(t: &[u8], bpp: u32) -> Vec<u8> {
    if bpp == 8 {
        t.to_vec()
    } else {
        t.chunks(2).map(|p| (p[0] & 15) | ((p[1] & 15) << 4)).collect()
    }
}

fn flip_h(t: &[u8]) -> Vec<u8> {
    (0..8).flat_map(|y| (0..8).rev().map(move |x| (y, x))).map(|(y, x)| t[y * 8 + x]).collect()
}
fn flip_v(t: &[u8]) -> Vec<u8> {
    (0..8).rev().flat_map(|y| (0..8).map(move |x| (y, x))).map(|(y, x)| t[y * 8 + x]).collect()
}

/// Build a de-duplicated tile set (tile 0 is always empty) and a map of
/// screen entries. Flipped copies of a tile reuse it with the flip bits.
/// Maps wider than 32 tiles are laid out in 32x32 screen blocks, the way the
/// GBA expects 64x32 / 64x64 backgrounds.
/// Where tile (tx, ty) goes in the map array. Maps up to 32 tiles wide are
/// plain row-major; 64-tile-wide maps are two 32x32 screen blocks side by
/// side (and stacked for 64x64), as the GBA hardware requires.
pub fn map_index(cols: u32, tx: u32, ty: u32) -> usize {
    if cols <= 32 {
        (ty * cols + tx) as usize
    } else {
        let sb = (ty / 32) * 2 + tx / 32;
        (sb * 1024 + (ty % 32) * 32 + tx % 32) as usize
    }
}

fn build_map(indices: &[u8], w: u32, h: u32) -> (Vec<Vec<u8>>, Vec<u16>) {
    let mut unique: Vec<Vec<u8>> = vec![vec![0; 64]];
    let mut lookup: HashMap<Vec<u8>, u16> = HashMap::new();
    lookup.insert(vec![0; 64], 0);
    let cols = w / 8;
    let rows = h / 8;
    let mut entries = vec![0u16; (cols * rows) as usize];
    for ty in 0..rows {
        for tx in 0..cols {
            let t = extract_tile(indices, w, tx * 8, ty * 8);
            let entry = if let Some(&i) = lookup.get(&t) {
                i
            } else if let Some(&i) = lookup.get(&flip_h(&t)) {
                i | 0x0400
            } else if let Some(&i) = lookup.get(&flip_v(&t)) {
                i | 0x0800
            } else if let Some(&i) = lookup.get(&flip_v(&flip_h(&t))) {
                i | 0x0C00
            } else {
                let i = unique.len() as u16;
                unique.push(t.clone());
                lookup.insert(t, i);
                i
            };
            entries[map_index(cols, tx, ty)] = entry;
        }
    }
    (unique, entries)
}

/// Result of converting the whole `assets/` folder.
#[derive(Debug, Clone, Default)]
pub struct AssetOutput {
    pub header: String,
    pub source: String,
    pub images: Vec<ConvertedImage>,
    pub errors: Vec<AssetError>,
}

/// Convert every PNG of a project. `assets_dir` is used to detect files in
/// the `backgrounds/` sub-folder.
pub fn convert_all(assets_dir: &Path, files: &[PathBuf]) -> AssetOutput {
    let mut out = AssetOutput::default();
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    for file in files {
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let is_bg = file
            .strip_prefix(assets_dir)
            .ok()
            .and_then(|rel| rel.components().next())
            .map(|c| c.as_os_str() == "backgrounds")
            .unwrap_or(false);
        let img = match image::open(file) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                out.errors.push(AssetError { file: file.clone(), message: format!("cannot read image: {e}") });
                continue;
            }
        };
        match convert_image(&img, &stem, is_bg) {
            Ok(c) => {
                if let Some(prev) = seen.insert(c.ident.clone(), file.clone()) {
                    out.errors.push(AssetError {
                        file: file.clone(),
                        message: format!("has the same C name `{}` as {}; rename one of them", c.ident, prev.display()),
                    });
                    continue;
                }
                out.images.push(c);
            }
            Err(message) => out.errors.push(AssetError { file: file.clone(), message }),
        }
    }
    let (h, c) = generate_c(&out.images);
    out.header = h;
    out.source = c;
    out
}

/// Produce `assets.h` and `assets.c`.
pub fn generate_c(images: &[ConvertedImage]) -> (String, String) {
    let mut h = String::new();
    let mut c = String::new();
    h.push_str("/* Generated by GBADE from the PNG files in assets/. Do not edit:\n * this file is rewritten at every build. */\n");
    h.push_str("#ifndef GBADE_ASSETS_H\n#define GBADE_ASSETS_H\n\n#include <gba/types.h>\n\n");
    c.push_str("/* Generated by GBADE from the PNG files in assets/. Do not edit. */\n#include \"assets.h\"\n\n");

    for img in images {
        let id = &img.ident;
        let up = id.to_ascii_uppercase();
        let _ = writeln!(h, "/* ---- {id}: {}x{} pixels, {} colour(s) ---- */", img.width, img.height, img.colors);
        let _ = writeln!(h, "#define {up}_WIDTH {}", img.width);
        let _ = writeln!(h, "#define {up}_HEIGHT {}", img.height);
        let _ = writeln!(h, "#define {up}_COLORS {}", img.colors);
        if img.bpp > 0 {
            let _ = writeln!(h, "#define {up}_BPP {}", img.bpp);
            let _ = writeln!(h, "#define {up}_TILE_COUNT {}", img.tile_count);
            let _ = writeln!(h, "#define {up}_FRAME_WIDTH {}", img.frame.0);
            let _ = writeln!(h, "#define {up}_FRAME_HEIGHT {}", img.frame.1);
            let _ = writeln!(h, "#define {up}_FRAME_TILES {}", (img.frame.0 / 8) * (img.frame.1 / 8));
            let _ = writeln!(h, "#define {up}_PAL_SIZE {} /* bytes */", img.palette.len() * 2);
            let _ = writeln!(h, "#define {up}_TILES_SIZE {} /* bytes */", img.tiles.len());
            let _ = writeln!(h, "extern const u16 {id}_pal[{}];", img.palette.len());
            let _ = writeln!(h, "extern const u32 {id}_tiles[{}];", img.tiles.len() / 4);
            write_array_u16(&mut c, &format!("{id}_pal"), &img.palette);
            let words: Vec<u32> =
                img.tiles.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            write_array_u32(&mut c, &format!("{id}_tiles"), &words);
        }
        if let Some(map) = &img.map {
            let _ = writeln!(h, "#define {up}_MAP_WIDTH {} /* tiles */", img.map_size.0);
            let _ = writeln!(h, "#define {up}_MAP_HEIGHT {} /* tiles */", img.map_size.1);
            let _ = writeln!(h, "#define {up}_MAP_SIZE {} /* bytes */", map.len() * 2);
            let _ = writeln!(h, "extern const u16 {id}_map[{}];", map.len());
            write_array_u16(&mut c, &format!("{id}_map"), map);
        }
        if let Some(bmp) = &img.bitmap {
            let _ = writeln!(h, "#define {up}_BITMAP_SIZE {} /* bytes */", bmp.len() * 2);
            let _ = writeln!(h, "extern const u16 {id}_bitmap[{}];", bmp.len());
            write_array_u16(&mut c, &format!("{id}_bitmap"), bmp);
        }
        h.push('\n');
    }
    h.push_str("#endif\n");
    (h, c)
}

fn write_array_u16(out: &mut String, name: &str, data: &[u16]) {
    let _ = writeln!(out, "const u16 {name}[{}] __attribute__((aligned(4))) = {{", data.len());
    for chunk in data.chunks(16) {
        out.push_str("    ");
        for v in chunk {
            let _ = write!(out, "0x{v:04X},");
        }
        out.push('\n');
    }
    out.push_str("};\n\n");
}

fn write_array_u32(out: &mut String, name: &str, data: &[u32]) {
    let _ = writeln!(out, "const u32 {name}[{}] __attribute__((aligned(4))) = {{", data.len());
    for chunk in data.chunks(8) {
        out.push_str("    ");
        for v in chunk {
            let _ = write!(out, "0x{v:08X},");
        }
        out.push('\n');
    }
    out.push_str("};\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn img(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| Rgba(f(x, y)))
    }

    #[test]
    fn colour_conversion() {
        assert_eq!(rgb_to_bgr555(255, 0, 0), 0x001F);
        assert_eq!(rgb_to_bgr555(0, 255, 0), 0x03E0);
        assert_eq!(rgb_to_bgr555(0, 0, 255), 0x7C00);
        assert_eq!(bgr555_to_rgb(0x7FFF), (255, 255, 255));
        assert_eq!(c_ident("Hero Walk-1"), "hero_walk_1");
        assert_eq!(c_ident("1up"), "_1up");
    }

    #[test]
    fn small_sprite_is_4bpp_with_transparency() {
        // 8x8: left half red, right half transparent.
        let i = img(8, 8, |x, _| if x < 4 { [255, 0, 0, 255] } else { [0, 0, 0, 0] });
        let c = convert_image(&i, "dot", false).unwrap();
        assert_eq!(c.bpp, 4);
        assert_eq!(c.colors, 1);
        assert_eq!(c.palette.len(), 16);
        assert_eq!(c.palette[1], 0x001F);
        assert_eq!(c.tile_count, 1);
        // Row: pixels 0..3 = index 1, 4..7 = 0 -> bytes 0x11 0x11 0x00 0x00.
        assert_eq!(&c.tiles[0..4], &[0x11, 0x11, 0x00, 0x00]);
        assert!(c.bitmap.is_some());
    }

    #[test]
    fn magenta_is_transparent() {
        let i = img(8, 8, |_, _| [255, 0, 255, 255]);
        let c = convert_image(&i, "m", false).unwrap();
        assert_eq!(c.colors, 0);
        assert!(c.tiles.iter().all(|&b| b == 0));
    }

    #[test]
    fn many_colours_use_8bpp() {
        let i = img(16, 8, |x, y| [(x * 16) as u8, (y * 30) as u8, 100, 255]);
        let c = convert_image(&i, "rainbow", false).unwrap();
        assert!(c.colors > 15);
        assert_eq!(c.bpp, 8);
        assert_eq!(c.palette.len(), 256);
        assert_eq!(c.tiles.len(), 2 * 64);
    }

    #[test]
    fn sprite_sheet_frames_are_contiguous() {
        // 32x16 sheet of two 16x16 frames: frame 0 colour A, frame 1 colour B.
        let i = img(32, 16, |x, _| if x < 16 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let c = convert_image(&i, "hero.16x16", false).unwrap();
        assert_eq!(c.ident, "hero");
        assert_eq!(c.frame, (16, 16));
        assert_eq!(c.tile_count, 8);
        // First 4 tiles (frame 0) all index 1, next 4 all index 2.
        assert!(c.tiles[..128].iter().all(|&b| b == 0x11));
        assert!(c.tiles[128..].iter().all(|&b| b == 0x22));
    }

    #[test]
    fn bad_frame_size_is_an_error() {
        let i = img(24, 16, |_, _| [1, 2, 3, 255]);
        assert!(convert_image(&i, "x.16x16", false).is_err());
    }

    #[test]
    fn background_dedups_tiles_and_flips() {
        // 16x8: tile A then its horizontal mirror.
        let i = img(16, 8, |x, _| {
            let lx = if x < 8 { x } else { 15 - x };
            if lx == 0 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }
        });
        let c = convert_image(&i, "bg", true).unwrap();
        let map = c.map.unwrap();
        assert_eq!(map.len(), 2);
        // Tile 0 is the reserved empty tile, so A is tile 1 and B = A flipped.
        assert_eq!(map[0], 1);
        assert_eq!(map[1], 1 | 0x0400);
        assert_eq!(c.tile_count, 2);
    }

    #[test]
    fn wide_background_uses_screenblocks() {
        // 512x256 = 64x32 tiles: tile column 32 must start the second screen block.
        let i = img(512, 256, |x, _| if x / 8 == 32 { [255, 255, 255, 255] } else { [0, 0, 0, 255] });
        let c = convert_image(&i, "wide", true).unwrap();
        let map = c.map.unwrap();
        assert_eq!(map.len(), 64 * 32);
        assert_eq!(c.map_size, (64, 32));
        let black = map[0];
        let white = map[1024];
        assert_ne!(black, white);
        assert_eq!(map[1024 + 32], white, "row 1 of screen block 1 starts at +32");
        assert_eq!(map[31], black);
        assert_eq!(map_index(64, 33, 1), 1024 + 32 + 1);
        assert_eq!(map_index(30, 2, 1), 32);
        // 400 px wide is not a valid GBA map width.
        assert!(convert_image(&img(400, 256, |_, _| [0, 0, 0, 255]), "bad", true).is_err());
    }

    #[test]
    fn non_multiple_of_8_background_errors() {
        let i = img(10, 8, |_, _| [0, 0, 0, 255]);
        let e = convert_image(&i, "bg", true).unwrap_err();
        assert!(e.contains("multiple of 8"));
    }

    #[test]
    fn generated_c_mentions_symbols() {
        let i = img(8, 8, |_, _| [0, 255, 0, 255]);
        let c = convert_image(&i, "Leaf", false).unwrap();
        let (h, src) = generate_c(&[c]);
        assert!(h.contains("extern const u16 leaf_pal[16];"));
        assert!(h.contains("#define LEAF_TILE_COUNT 1"));
        assert!(src.contains("const u32 leaf_tiles[8]"));
        assert!(src.contains("leaf_bitmap"));
    }
}
