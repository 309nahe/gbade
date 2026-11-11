//! Turning the linked ELF file into a `.gba` ROM image.
//!
//! The linker produces an ELF file: a container describing where each chunk
//! of the program must go in memory. A GBA cartridge is simply the raw bytes
//! as they appear at address 0x08000000. We copy every loadable segment to its
//! *load* address (LMA, `p_paddr`) relative to the start of ROM, then fill in
//! the cartridge header (logo, title, checksum) like the classic `gbafix`
//! tool does.

use object::read::elf::{FileHeader, ProgramHeader};
use object::{LittleEndian, elf};

pub use crate::nintendo_logo::NINTENDO_LOGO;

/// Start of cartridge ROM in the GBA address space.
pub const ROM_BASE: u32 = 0x0800_0000;
/// Maximum size of a GBA ROM (32 MB).
pub const ROM_MAX: u32 = 32 * 1024 * 1024;

/// What to write in the cartridge header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderInfo {
    /// Up to 12 characters.
    pub title: String,
    /// Exactly 4 characters (padded/truncated).
    pub game_code: String,
    /// Exactly 2 characters.
    pub maker_code: String,
    pub version: u8,
}

impl Default for HeaderInfo {
    fn default() -> Self {
        HeaderInfo { title: "GBADE".into(), game_code: "GBDE".into(), maker_code: "01".into(), version: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomError(pub String);

impl std::fmt::Display for RomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RomError {}

/// Convert a linked ELF file into a ROM image with a valid header.
pub fn elf_to_rom(elf_data: &[u8], header: &HeaderInfo) -> Result<Vec<u8>, RomError> {
    let file = elf::FileHeader32::<LittleEndian>::parse(elf_data)
        .map_err(|e| RomError(format!("not a valid 32-bit ELF file: {e}")))?;
    let endian = file.endian().map_err(|e| RomError(e.to_string()))?;
    let phdrs = file.program_headers(endian, elf_data).map_err(|e| RomError(e.to_string()))?;

    let mut rom: Vec<u8> = Vec::new();
    for ph in phdrs {
        if ph.p_type(endian) != elf::PT_LOAD || ph.p_filesz(endian) == 0 {
            continue;
        }
        let lma = ph.p_paddr(endian);
        let size = ph.p_filesz(endian);
        if !(ROM_BASE..ROM_BASE + ROM_MAX).contains(&lma) {
            return Err(RomError(format!(
                "a segment with initialised data must be stored in ROM, but its load address is {lma:#010x}"
            )));
        }
        let offset = (lma - ROM_BASE) as usize;
        let end = offset + size as usize;
        if end > ROM_MAX as usize {
            return Err(RomError("the game is larger than 32 MB, the maximum size of a GBA cartridge".into()));
        }
        let data = ph.data(endian, elf_data).map_err(|_| RomError("segment data out of bounds".into()))?;
        if rom.len() < end {
            rom.resize(end, 0);
        }
        rom[offset..end].copy_from_slice(data);
    }
    if rom.len() < 0xC0 {
        return Err(RomError("no code found at 0x08000000: is the startup code (crt0) linked?".into()));
    }
    // Pad to a multiple of 4 bytes; some flash carts and emulators like it.
    while rom.len() % 4 != 0 {
        rom.push(0);
    }
    fix_header(&mut rom, header);
    Ok(rom)
}

/// Write the logo, title, codes and checksum into the first 192 bytes.
pub fn fix_header(rom: &mut [u8], header: &HeaderInfo) {
    assert!(rom.len() >= 0xC0);
    rom[0x04..0xA0].copy_from_slice(&NINTENDO_LOGO);
    write_padded(&mut rom[0xA0..0xAC], &header.title.to_ascii_uppercase());
    write_padded(&mut rom[0xAC..0xB0], &header.game_code.to_ascii_uppercase());
    write_padded(&mut rom[0xB0..0xB2], &header.maker_code.to_ascii_uppercase());
    rom[0xB2] = 0x96;
    rom[0xB3] = 0;
    rom[0xB4] = 0;
    rom[0xB5..0xBC].fill(0);
    rom[0xBC] = header.version;
    rom[0xBD] = header_checksum(rom);
}

/// The "complement check" byte at 0xBD: the BIOS refuses to boot if wrong.
pub fn header_checksum(rom: &[u8]) -> u8 {
    let mut chk: u8 = 0;
    for &b in &rom[0xA0..0xBD] {
        chk = chk.wrapping_sub(b);
    }
    chk.wrapping_sub(0x19)
}

/// Check a ROM header the way the GBA BIOS does. Returns a list of problems.
pub fn validate_header(rom: &[u8]) -> Vec<String> {
    let mut problems = Vec::new();
    if rom.len() < 0xC0 {
        problems.push("ROM smaller than its header".into());
        return problems;
    }
    if rom[0x04..0xA0] != NINTENDO_LOGO {
        problems.push("logo bitmap is wrong (real hardware will not boot)".into());
    }
    if rom[0xB2] != 0x96 {
        problems.push("fixed byte at 0xB2 is not 0x96".into());
    }
    if rom[0xBD] != header_checksum(rom) {
        problems.push("header checksum is wrong (real hardware will not boot)".into());
    }
    problems
}

fn write_padded(dst: &mut [u8], s: &str) {
    dst.fill(0);
    for (d, c) in dst.iter_mut().zip(s.bytes().filter(|b| b.is_ascii() && *b >= 0x20)) {
        *d = c;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_matches_reference() {
        // Header from a known-good ROM: all zero title/codes, version 0.
        let mut rom = vec![0u8; 0xC0];
        fix_header(&mut rom, &HeaderInfo { title: String::new(), game_code: String::new(), maker_code: String::new(), version: 0 });
        // Sum of 0xA0..0xBD is just 0x96, so chk = -0x96 - 0x19 = 0x51.
        assert_eq!(rom[0xBD], 0x51);
        assert!(validate_header(&rom).is_empty());
    }

    #[test]
    fn header_fields_written() {
        let mut rom = vec![0u8; 0x100];
        fix_header(&mut rom, &HeaderInfo { title: "hello world!!!".into(), game_code: "AB".into(), maker_code: "7".into(), version: 3 });
        assert_eq!(&rom[0xA0..0xAC], b"HELLO WORLD!");
        assert_eq!(&rom[0xAC..0xB0], b"AB\0\0");
        assert_eq!(&rom[0xB0..0xB2], b"7\0");
        assert_eq!(rom[0xBC], 3);
        assert!(validate_header(&rom).is_empty());
        rom[0xA0] = b'X';
        assert_eq!(validate_header(&rom).len(), 1);
    }

    #[test]
    fn rejects_garbage() {
        assert!(elf_to_rom(b"not an elf", &HeaderInfo::default()).is_err());
    }
}
