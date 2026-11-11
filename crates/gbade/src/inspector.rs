//! The hardware inspector: look inside the running GBA.
//!
//! - CPU: registers, flags, current function and disassembly.
//! - I/O: the important hardware registers, decoded and explained.
//! - Palettes: the 512 colours.
//! - Tiles: the contents of VRAM as 8x8 tiles.
//! - Sprites: the 128 OAM entries.
//! - Memory: a hex viewer for every memory region.

use crate::session::Session;
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions, Ui, Vec2};
use gbade_toolchain::assets::bgr555_to_rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Cpu,
    Io,
    Palette,
    Tiles,
    Sprites,
    Memory,
}

pub struct Inspector {
    pub tab: Tab,
    tiles_tex: Option<TextureHandle>,
    tiles_block: usize,
    tiles_palbank: usize,
    tiles_8bpp: bool,
    mem_region: usize,
    mem_offset: u32,
    mem_goto: String,
}

impl Default for Inspector {
    fn default() -> Self {
        Inspector {
            tab: Tab::Cpu,
            tiles_tex: None,
            tiles_block: 0,
            tiles_palbank: 0,
            tiles_8bpp: false,
            mem_region: 1,
            mem_offset: 0,
            mem_goto: String::new(),
        }
    }
}

/// (name, start, size) of memory regions.
pub const REGIONS: &[(&str, u32, u32)] = &[
    ("EWRAM (256 KB)", 0x0200_0000, 0x40000),
    ("IWRAM (32 KB)", 0x0300_0000, 0x8000),
    ("I/O registers", 0x0400_0000, 0x400),
    ("Palette RAM", 0x0500_0000, 0x400),
    ("VRAM (96 KB)", 0x0600_0000, 0x18000),
    ("OAM (sprites)", 0x0700_0000, 0x400),
    ("ROM", 0x0800_0000, 0x0200_0000),
    ("SRAM (save)", 0x0E00_0000, 0x8000),
];

pub fn color32(c: u16) -> Color32 {
    let (r, g, b) = bgr555_to_rgb(c);
    Color32::from_rgb(r, g, b)
}

impl Inspector {
    pub fn ui(&mut self, ui: &mut Ui, s: &Session) {
        ui.horizontal_wrapped(|ui| {
            for (t, name) in [
                (Tab::Cpu, "CPU"),
                (Tab::Io, "I/O"),
                (Tab::Palette, "Palettes"),
                (Tab::Tiles, "Tiles"),
                (Tab::Sprites, "Sprites"),
                (Tab::Memory, "Memory"),
            ] {
                ui.selectable_value(&mut self.tab, t, name);
            }
        });
        ui.separator();
        match self.tab {
            Tab::Cpu => cpu_ui(ui, s),
            Tab::Io => io_ui(ui, s),
            Tab::Palette => palette_ui(ui, s),
            Tab::Tiles => self.tiles_ui(ui, s),
            Tab::Sprites => sprites_ui(ui, s),
            Tab::Memory => self.memory_ui(ui, s),
        }
    }

    fn tiles_ui(&mut self, ui: &mut Ui, s: &Session) {
        ui.horizontal(|ui| {
            ui.label("Block:");
            egui::ComboBox::from_id_salt("tileblock")
                .selected_text(tile_block_name(self.tiles_block))
                .show_ui(ui, |ui| {
                    for b in 0..6 {
                        ui.selectable_value(&mut self.tiles_block, b, tile_block_name(b));
                    }
                });
            ui.checkbox(&mut self.tiles_8bpp, "256 colours");
            if !self.tiles_8bpp {
                ui.label("Palette bank:");
                ui.add(egui::DragValue::new(&mut self.tiles_palbank).range(0..=15));
            }
        });
        ui.label(
            RichText::new("Each 16 KB block holds 512 tiles (4bpp) or 256 tiles (8bpp). Sprite tiles are in blocks 4-5.")
                .small()
                .weak(),
        );
        let base = 0x0600_0000 + self.tiles_block as u32 * 0x4000;
        let data = s.emu.read_block(base, 0x4000);
        let pal_base = if self.tiles_block >= 4 { 0x0500_0200 } else { 0x0500_0000 };
        let pal_data = s.emu.read_block(pal_base, 512);
        let pal: Vec<u16> = pal_data.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let (cols, rows, tile_bytes) = if self.tiles_8bpp { (16, 16, 64) } else { (16, 32, 32) };
        let (w, h) = (cols * 8, rows * 8);
        let mut img = ColorImage::new([w, h], vec![Color32::BLACK; w * h]);
        for t in 0..cols * rows {
            let tx = (t % cols) * 8;
            let ty = (t / cols) * 8;
            for py in 0..8 {
                for px in 0..8 {
                    let idx = if self.tiles_8bpp {
                        data[t * tile_bytes + py * 8 + px] as usize
                    } else {
                        let b = data[t * tile_bytes + py * 4 + px / 2];
                        let nib = if px % 2 == 0 { b & 15 } else { b >> 4 } as usize;
                        if nib == 0 { 0 } else { self.tiles_palbank * 16 + nib }
                    };
                    let c = if idx == 0 { Color32::from_gray(40) } else { color32(pal[idx]) };
                    img.pixels[(ty + py) * w + tx + px] = c;
                }
            }
        }
        let tex = self.tiles_tex.get_or_insert_with(|| {
            ui.ctx().load_texture("tiles", ColorImage::new([1, 1], vec![Color32::BLACK]), TextureOptions::NEAREST)
        });
        tex.set(img, TextureOptions::NEAREST);
        let scale = ((ui.available_width() / w as f32).floor()).clamp(1.0, 3.0);
        let resp = ui.image((tex.id(), Vec2::new(w as f32 * scale, h as f32 * scale)));
        if let Some(p) = resp.hover_pos() {
            let lx = ((p.x - resp.rect.left()) / scale) as usize / 8;
            let ly = ((p.y - resp.rect.top()) / scale) as usize / 8;
            let t = ly * cols + lx;
            resp.on_hover_text(format!(
                "tile {t} (address {:#010x})",
                base + (t * tile_bytes) as u32
            ));
        }
    }

    fn memory_ui(&mut self, ui: &mut Ui, s: &Session) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("memregion").selected_text(REGIONS[self.mem_region].0).show_ui(ui, |ui| {
                for (i, r) in REGIONS.iter().enumerate() {
                    if ui.selectable_value(&mut self.mem_region, i, r.0).clicked() {
                        self.mem_offset = 0;
                    }
                }
            });
            ui.label("Go to:");
            let r = ui.add(egui::TextEdit::singleline(&mut self.mem_goto).desired_width(110.0).hint_text("0x03000000 or name"));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let target = parse_addr(&self.mem_goto)
                    .or_else(|| s.debug.as_ref().and_then(|d| d.symbol(self.mem_goto.trim())).map(|sym| sym.address));
                if let Some(addr) = target {
                    if let Some(i) = REGIONS.iter().position(|(_, b, n)| addr >= *b && addr < b + n) {
                        self.mem_region = i;
                        self.mem_offset = (addr - REGIONS[i].1) & !0xF;
                    }
                }
            }
        });
        let (_, base, size) = REGIONS[self.mem_region];
        let row_count = (size / 16) as usize;
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        let target_row = (self.mem_offset / 16) as usize;
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(("mem", self.mem_region));
        if !self.mem_goto.is_empty() && self.mem_offset > 0 {
            area = area.vertical_scroll_offset(target_row as f32 * (row_h + ui.spacing().item_spacing.y));
            self.mem_offset = 0;
        }
        area.show_rows(ui, row_h, row_count, |ui, range| {
            for row in range {
                let addr = base + row as u32 * 16;
                let bytes = s.emu.read_block(addr, 16);
                let mut line = format!("{addr:08X}  ");
                for (i, b) in bytes.iter().enumerate() {
                    line.push_str(&format!("{b:02X} "));
                    if i == 7 {
                        line.push(' ');
                    }
                }
                line.push(' ');
                for b in &bytes {
                    line.push(if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '.' });
                }
                let resp = ui.label(RichText::new(line).monospace());
                if let Some(sym) = s.debug.as_ref().and_then(|d| d.symbol_at(addr)) {
                    resp.on_hover_text(format!("{} (+{:#x})", sym.name, addr - sym.address));
                }
            }
        });
    }
}

/// Add `<function>` after addresses in disassembly that start a function.
pub fn annotate_targets(text: &str, debug: Option<&gbade_toolchain::DebugInfo>) -> String {
    let Some(d) = debug else { return text.to_string() };
    let mut out = text.to_string();
    if let Some(pos) = text.find("0x") {
        let hex: String = text[pos + 2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if let Ok(addr) = u32::from_str_radix(&hex, 16) {
            if let Some(sym) = d.symbols.iter().find(|s| s.is_function && s.address == addr & !1) {
                out.push_str(&format!("  <{}>", sym.name));
            }
        }
    }
    out
}

fn tile_block_name(b: usize) -> String {
    if b < 4 { format!("BG block {b}") } else { format!("Sprite block {}", b - 4) }
}

fn parse_addr(s: &str) -> Option<u32> {
    let t = s.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    u32::from_str_radix(t, 16).ok().filter(|_| !t.is_empty() && t.len() <= 8)
}

fn cpu_ui(ui: &mut Ui, s: &Session) {
    let cpu = s.emu.cpu_state();
    let debug = s.debug.as_ref();
    if cpu.pc() < 0x4000 {
        ui.label(
            RichText::new("The CPU is inside the BIOS (for example sleeping in vsync() until the next VBlank).")
                .color(Color32::from_rgb(140, 170, 230)),
        );
    }
    if let Some(f) = debug.and_then(|d| d.function_at(cpu.pc())) {
        ui.label(format!("In function: {}", f.name));
    }
    if let Some((file, line)) = debug.and_then(|d| d.line_for_address(cpu.pc())) {
        ui.label(format!("Source: {}:{line}", file.file_name().unwrap_or_default().to_string_lossy()));
    }
    ui.label(format!(
        "Mode: {} ({})   Flags: {}{}{}{}   IRQ {}",
        cpu.mode_name(),
        if cpu.thumb() { "Thumb, 16-bit instructions" } else { "ARM, 32-bit instructions" },
        if cpu.cpsr & (1 << 31) != 0 { "N" } else { "-" },
        if cpu.cpsr & (1 << 30) != 0 { "Z" } else { "-" },
        if cpu.cpsr & (1 << 29) != 0 { "C" } else { "-" },
        if cpu.cpsr & (1 << 28) != 0 { "V" } else { "-" },
        if cpu.cpsr & (1 << 7) != 0 { "off" } else { "on" },
    ));
    egui::Grid::new("regs").num_columns(4).striped(true).show(ui, |ui| {
        for i in 0..16 {
            let name = match i {
                13 => "sp".to_string(),
                14 => "lr".to_string(),
                15 => "pc".to_string(),
                n => format!("r{n}"),
            };
            let v = cpu.regs[i];
            let resp = ui.label(RichText::new(format!("{name:>3} {v:08X}")).monospace());
            if let Some(sym) = debug.and_then(|d| d.symbol_at(v)) {
                resp.on_hover_text(format!("{} + {:#x}", sym.name, v - sym.address));
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
    });
    ui.separator();
    ui.label(RichText::new("Disassembly").strong());
    let step = if cpu.thumb() { 2 } else { 4 };
    let mut addr = cpu.pc().saturating_sub(step * 4);
    for _ in 0..12 {
        let (text, size) = s.emu.disassemble(addr, cpu.thumb());
        let text = annotate_targets(&text, debug);
        let marker = if addr == cpu.pc() { "▶" } else { " " };
        let label = debug.and_then(|d| d.symbols.iter().find(|sy| sy.is_function && sy.address == addr)).map(|sy| format!("{}:", sy.name));
        if let Some(l) = label {
            ui.label(RichText::new(l).monospace().color(Color32::from_rgb(220, 200, 120)));
        }
        let rt = RichText::new(format!("{marker} {addr:08X}  {text}")).monospace();
        ui.label(if addr == cpu.pc() { rt.color(Color32::from_rgb(255, 210, 0)) } else { rt });
        addr += size;
    }
}

struct IoReg {
    name: &'static str,
    addr: u32,
    help: &'static str,
}

const IO_REGS: &[IoReg] = &[
    IoReg { name: "DISPCNT", addr: 0x0400_0000, help: "Display control: video mode (0-5), which backgrounds and sprites are on." },
    IoReg { name: "DISPSTAT", addr: 0x0400_0004, help: "Display status: VBlank/HBlank flags and their interrupt enables." },
    IoReg { name: "VCOUNT", addr: 0x0400_0006, help: "The scanline being drawn right now (0-159 visible, 160-227 VBlank)." },
    IoReg { name: "BG0CNT", addr: 0x0400_0008, help: "Background 0 control: priority, tile block, map block, colours, size." },
    IoReg { name: "BG1CNT", addr: 0x0400_000A, help: "Background 1 control." },
    IoReg { name: "BG2CNT", addr: 0x0400_000C, help: "Background 2 control." },
    IoReg { name: "BG3CNT", addr: 0x0400_000E, help: "Background 3 control." },
    IoReg { name: "BG0HOFS", addr: 0x0400_0010, help: "Background 0 horizontal scroll (write-only on hardware)." },
    IoReg { name: "BG0VOFS", addr: 0x0400_0012, help: "Background 0 vertical scroll." },
    IoReg { name: "BLDCNT", addr: 0x0400_0050, help: "Blending: which layers are blended and how (alpha, fade)." },
    IoReg { name: "SOUNDCNT_X", addr: 0x0400_0084, help: "Sound master enable (bit 7) and channel status." },
    IoReg { name: "TM0CNT", addr: 0x0400_0102, help: "Timer 0 control: speed, cascade, interrupt, enable." },
    IoReg { name: "TM0D", addr: 0x0400_0100, help: "Timer 0 counter." },
    IoReg { name: "KEYINPUT", addr: 0x0400_0130, help: "Buttons: a bit is 0 when the button is PRESSED." },
    IoReg { name: "IE", addr: 0x0400_0200, help: "Interrupt enable: which interrupts may fire." },
    IoReg { name: "IF", addr: 0x0400_0202, help: "Interrupt flags: which interrupts are waiting." },
    IoReg { name: "IME", addr: 0x0400_0208, help: "Interrupt master enable: 1 = interrupts allowed." },
];

/// Plain-English decoding of the most important registers.
pub fn decode_io(name: &str, v: u16) -> String {
    match name {
        "DISPCNT" => {
            let mut parts = vec![format!("mode {}", v & 7)];
            for (bit, n) in [(8, "BG0"), (9, "BG1"), (10, "BG2"), (11, "BG3"), (12, "sprites")] {
                if v & (1 << bit) != 0 {
                    parts.push(n.to_string());
                }
            }
            if v & 0x40 != 0 {
                parts.push("1D sprite tiles".into());
            }
            if v & 0x80 != 0 {
                parts.push("FORCED BLANK (screen white)".into());
            }
            if v & 0x10 != 0 {
                parts.push("page 1".into());
            }
            parts.join(", ")
        }
        "DISPSTAT" => format!(
            "{}{}{}",
            if v & 1 != 0 { "in VBlank " } else { "" },
            if v & 8 != 0 { "VBlank IRQ on " } else { "" },
            if v & 0x10 != 0 { "HBlank IRQ on" } else { "" }
        ),
        n if n.starts_with("BG") && n.ends_with("CNT") => format!(
            "priority {}, tiles in block {}, map in screen block {}, {}, size {}",
            v & 3,
            (v >> 2) & 3,
            (v >> 8) & 31,
            if v & 0x80 != 0 { "256 colours" } else { "16 colours" },
            ["32x32", "64x32", "32x64", "64x64"][(v >> 14) as usize & 3]
        ),
        "KEYINPUT" => {
            let names = ["A", "B", "Select", "Start", "Right", "Left", "Up", "Down", "R", "L"];
            let pressed: Vec<&str> = names.iter().enumerate().filter(|(i, _)| v & (1 << i) == 0).map(|(_, n)| *n).collect();
            if pressed.is_empty() { "no button pressed".into() } else { format!("pressed: {}", pressed.join(" ")) }
        }
        "IE" | "IF" => {
            let names = ["VBlank", "HBlank", "VCount", "Timer0", "Timer1", "Timer2", "Timer3", "Serial", "DMA0", "DMA1", "DMA2", "DMA3", "Keypad", "GamePak"];
            let on: Vec<&str> = names.iter().enumerate().filter(|(i, _)| v & (1 << i) != 0).map(|(_, n)| *n).collect();
            if on.is_empty() { "none".into() } else { on.join(" ") }
        }
        "IME" => if v & 1 != 0 { "interrupts enabled".into() } else { "interrupts disabled".into() },
        "SOUNDCNT_X" => if v & 0x80 != 0 { "sound ON".into() } else { "sound off".into() },
        "TM0CNT" => {
            if v & 0x80 == 0 {
                "stopped".into()
            } else {
                format!("running, 1/{} clock{}", [1, 64, 256, 1024][(v & 3) as usize], if v & 0x40 != 0 { ", IRQ" } else { "" })
            }
        }
        _ => String::new(),
    }
}

fn io_ui(ui: &mut Ui, s: &Session) {
    ui.label(RichText::new("Hover a register for an explanation.").small().weak());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("io").num_columns(3).striped(true).show(ui, |ui| {
            for r in IO_REGS {
                let v = s.emu.read16(r.addr);
                ui.label(RichText::new(r.name).monospace()).on_hover_text(format!("{:#010x}\n{}", r.addr, r.help));
                ui.label(RichText::new(format!("{v:04X}")).monospace());
                ui.label(decode_io(r.name, v));
                ui.end_row();
            }
        });
    });
}

fn palette_ui(ui: &mut Ui, s: &Session) {
    let data = s.emu.read_block(0x0500_0000, 0x400);
    let colors: Vec<u16> = data.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let cell = ((ui.available_width() - 20.0) / 32.0).clamp(6.0, 14.0);
    ui.horizontal_top(|ui| {
        for (half, title) in [(0usize, "Backgrounds"), (1, "Sprites")] {
            ui.vertical(|ui| {
                ui.label(title);
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(cell * 16.0), egui::Sense::hover());
                let p = ui.painter_at(rect);
                for i in 0..256 {
                    let c = colors[half * 256 + i];
                    let r = egui::Rect::from_min_size(
                        rect.min + Vec2::new((i % 16) as f32 * cell, (i / 16) as f32 * cell),
                        Vec2::splat(cell - 1.0),
                    );
                    p.rect_filled(r, 0.0, color32(c));
                }
                if let Some(pos) = resp.hover_pos() {
                    let i = ((pos.y - rect.top()) / cell) as usize * 16 + ((pos.x - rect.left()) / cell) as usize;
                    if i < 256 {
                        let c = colors[half * 256 + i];
                        let (r, g, b) = (c & 31, (c >> 5) & 31, (c >> 10) & 31);
                        resp.on_hover_text(format!(
                            "index {i} (bank {}, colour {})\nvalue 0x{c:04X} = RGB15({r}, {g}, {b})",
                            i / 16,
                            i % 16
                        ));
                    }
                }
            });
        }
    });
    ui.label(RichText::new("Colour 0 of each bank is transparent. Background colour 0 is the backdrop.").small().weak());
}

fn sprites_ui(ui: &mut Ui, s: &Session) {
    let oam = s.emu.read_block(0x0700_0000, 0x400);
    let dispcnt = s.emu.read16(0x0400_0000);
    if dispcnt & 0x1000 == 0 {
        ui.colored_label(Color32::from_rgb(230, 180, 40), "Sprites are disabled (DCNT_OBJ is not set in REG_DISPCNT).");
    }
    let mut visible = 0;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("oam").num_columns(6).striped(true).show(ui, |ui| {
            ui.label("#");
            ui.label("x, y");
            ui.label("size");
            ui.label("tile");
            ui.label("pal");
            ui.label("flags");
            ui.end_row();
            for i in 0..128 {
                let a0 = u16::from_le_bytes([oam[i * 8], oam[i * 8 + 1]]);
                let a1 = u16::from_le_bytes([oam[i * 8 + 2], oam[i * 8 + 3]]);
                let a2 = u16::from_le_bytes([oam[i * 8 + 4], oam[i * 8 + 5]]);
                let affine = a0 & 0x100 != 0;
                if !affine && a0 & 0x200 != 0 {
                    continue; // hidden
                }
                visible += 1;
                let shape = (a0 >> 14) as usize & 3;
                let size = (a1 >> 14) as usize & 3;
                let dims = [[(8, 8), (16, 16), (32, 32), (64, 64)], [(16, 8), (32, 8), (32, 16), (64, 32)], [(8, 16), (8, 32), (16, 32), (32, 64)], [(0, 0); 4]][shape][size];
                let mut y = (a0 & 0xFF) as i32;
                if y >= 160 {
                    y -= 256;
                }
                let mut x = (a1 & 0x1FF) as i32;
                if x >= 240 {
                    x -= 512;
                }
                ui.label(i.to_string());
                ui.label(format!("{x}, {y}"));
                ui.label(format!("{}x{}", dims.0, dims.1));
                ui.label((a2 & 0x3FF).to_string());
                ui.label(((a2 >> 12) & 15).to_string());
                let mut flags = Vec::new();
                if a0 & 0x2000 != 0 {
                    flags.push("8bpp");
                }
                if affine {
                    flags.push("affine");
                } else {
                    if a1 & 0x1000 != 0 {
                        flags.push("hflip");
                    }
                    if a1 & 0x2000 != 0 {
                        flags.push("vflip");
                    }
                }
                if a0 & 0x400 != 0 {
                    flags.push("blend");
                }
                ui.label(format!("prio {} {}", (a2 >> 10) & 3, flags.join(" ")));
                ui.end_row();
            }
        });
        if visible == 0 {
            ui.label("No visible sprites (all 128 are hidden).");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_dispcnt() {
        assert_eq!(decode_io("DISPCNT", 0x0403), "mode 3, BG2");
        assert_eq!(decode_io("DISPCNT", 0x1140), "mode 0, BG0, sprites, 1D sprite tiles");
    }

    #[test]
    fn decodes_keys_and_irqs() {
        assert_eq!(decode_io("KEYINPUT", 0x03FF), "no button pressed");
        assert_eq!(decode_io("KEYINPUT", 0x03FE), "pressed: A");
        assert_eq!(decode_io("IE", 0x0009), "VBlank Timer0");
        assert_eq!(decode_io("BG0CNT", 0x1F0C), "priority 0, tiles in block 3, map in screen block 31, 16 colours, size 32x32");
    }

    #[test]
    fn parses_addresses() {
        assert_eq!(parse_addr("0x03000000"), Some(0x0300_0000));
        assert_eq!(parse_addr("6000000"), Some(0x0600_0000));
        assert_eq!(parse_addr("player"), None);
        assert_eq!(parse_addr(""), None);
    }
}
