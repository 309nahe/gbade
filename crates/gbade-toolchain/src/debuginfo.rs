//! Reading debug information from the linked ELF file.
//!
//! The compiler is run with `-g`, so the ELF contains:
//! - a **symbol table**: name, address and size of every function/variable,
//! - **DWARF line tables**: which source line produced which machine
//!   instruction.
//!
//! GBADE uses this to set breakpoints on source lines, to show where the game
//! is stopped ("main.c line 42"), and to name memory in the inspectors.

use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub address: u32,
    pub size: u32,
    pub is_function: bool,
}

/// One row of the line table: the instruction at `address` comes from
/// `file:line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRow {
    pub address: u32,
    pub file: usize,
    pub line: u32,
    pub is_stmt: bool,
}

/// Symbols and line tables of a linked game.
#[derive(Debug, Clone, Default)]
pub struct DebugInfo {
    pub symbols: Vec<Symbol>,
    pub files: Vec<PathBuf>,
    /// Sorted by address.
    pub rows: Vec<LineRow>,
    /// Section sizes, used for memory usage reports.
    pub sections: Vec<(String, u32, u32)>,
}

impl DebugInfo {
    /// Parse an ELF file. Missing DWARF is not an error (you just get no lines).
    pub fn from_elf(data: &[u8]) -> Result<DebugInfo, String> {
        let file = object::File::parse(data).map_err(|e| format!("cannot parse ELF: {e}"))?;
        let mut info = DebugInfo::default();

        for sym in file.symbols() {
            let Ok(name) = sym.name() else { continue };
            if name.is_empty() || name.starts_with('$') || name.starts_with(".L") {
                continue;
            }
            let is_function = sym.kind() == SymbolKind::Text;
            if !matches!(sym.kind(), SymbolKind::Text | SymbolKind::Data) && !name.starts_with("__") {
                continue;
            }
            // Thumb function addresses have bit 0 set in the symbol table.
            let address = (sym.address() as u32) & if is_function { !1 } else { !0 };
            info.symbols.push(Symbol { name: name.to_string(), address, size: sym.size() as u32, is_function });
        }
        info.symbols.sort_by_key(|s| (s.address, s.name.clone()));

        for sec in file.sections() {
            if let Ok(name) = sec.name() {
                info.sections.push((name.to_string(), sec.address() as u32, sec.size() as u32));
            }
        }

        let load = |id: gimli::SectionId| -> Result<Cow<'_, [u8]>, gimli::Error> {
            Ok(file.section_by_name(id.name()).and_then(|s| s.uncompressed_data().ok()).unwrap_or(Cow::Borrowed(&[])))
        };
        let dwarf_cow = gimli::DwarfSections::load(load).map_err(|e| e.to_string())?;
        let dwarf = dwarf_cow.borrow(|s| gimli::EndianSlice::new(s, gimli::LittleEndian));

        let mut units = dwarf.units();
        while let Ok(Some(header)) = units.next() {
            let Ok(unit) = dwarf.unit(header) else { continue };
            let Some(program) = unit.line_program.clone() else { continue };
            let comp_dir = unit
                .comp_dir
                .map(|d| PathBuf::from(d.to_string_lossy().into_owned()))
                .unwrap_or_default();
            let mut rows = program.rows();
            let mut file_map: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
            while let Ok(Some((header, row))) = rows.next_row() {
                if row.end_sequence() {
                    continue;
                }
                let Some(line) = row.line() else { continue };
                let file_index = row.file_index();
                let idx = match file_map.get(&file_index) {
                    Some(&i) => i,
                    None => {
                        let path = header
                            .file(file_index)
                            .map(|f| {
                                let mut p = comp_dir.clone();
                                if let Some(dir) = f.directory(header) {
                                    if let Ok(d) = dwarf.attr_string(&unit, dir) {
                                        p.push(d.to_string_lossy().as_ref());
                                    }
                                }
                                if let Ok(n) = dwarf.attr_string(&unit, f.path_name()) {
                                    p.push(n.to_string_lossy().as_ref());
                                }
                                p
                            })
                            .unwrap_or_default();
                        let path = normalize(&path);
                        let i = match info.files.iter().position(|f| *f == path) {
                            Some(i) => i,
                            None => {
                                info.files.push(path);
                                info.files.len() - 1
                            }
                        };
                        file_map.insert(file_index, i);
                        i
                    }
                };
                // The linker sets the address of removed (unused) functions to 0
                // in the debug info. Real GBA code is never below EWRAM.
                if (row.address() as u32) < 0x0200_0000 {
                    continue;
                }
                info.rows.push(LineRow {
                    address: row.address() as u32,
                    file: idx,
                    line: line.get() as u32,
                    is_stmt: row.is_stmt(),
                });
            }
        }
        info.rows.sort_by_key(|r| r.address);
        Ok(info)
    }

    /// The source location of the instruction at `address`.
    pub fn line_for_address(&self, address: u32) -> Option<(&Path, u32)> {
        let i = self.rows.partition_point(|r| r.address <= address);
        if i == 0 {
            return None;
        }
        let row = &self.rows[i - 1];
        // The address must be inside a known function, and the row must belong
        // to that same function (not the end of the previous one).
        match self.function_at(address) {
            Some(f) if row.address >= f.address => {}
            None if self.symbols.is_empty() => {}
            _ => return None,
        }
        Some((self.files[row.file].as_path(), row.line))
    }

    /// Addresses where a breakpoint on `file:line` should go. If the line has
    /// no code (comment, blank), the next line with code is used. Returns the
    /// line actually used and its addresses.
    pub fn addresses_for_line(&self, file: &Path, line: u32) -> Option<(u32, Vec<u32>)> {
        let file = normalize(file);
        let fidx: Vec<usize> =
            self.files.iter().enumerate().filter(|(_, f)| **f == file).map(|(i, _)| i).collect();
        if fidx.is_empty() {
            return None;
        }
        let candidates = |want: u32| -> Vec<u32> {
            let mut v: Vec<u32> = self
                .rows
                .iter()
                .filter(|r| r.is_stmt && r.line == want && fidx.contains(&r.file))
                .map(|r| r.address)
                .collect();
            v.sort();
            v.dedup();
            v
        };
        // Lines with code in this file, to find the next one.
        let mut lines: Vec<u32> =
            self.rows.iter().filter(|r| r.is_stmt && fidx.contains(&r.file) && r.line >= line).map(|r| r.line).collect();
        lines.sort();
        lines.dedup();
        for l in lines.into_iter().take(1) {
            let addrs = candidates(l);
            if !addrs.is_empty() {
                // Keep only the first address of each function (avoid stopping
                // several times for one line split into many instructions).
                let mut firsts: Vec<u32> = Vec::new();
                let mut seen_funcs: Vec<u32> = Vec::new();
                for a in addrs {
                    let f = self.function_at(a).map(|f| f.address).unwrap_or(a);
                    if !seen_funcs.contains(&f) {
                        seen_funcs.push(f);
                        firsts.push(a);
                    }
                }
                return Some((l, firsts));
            }
        }
        None
    }

    /// The function containing `address`.
    pub fn function_at(&self, address: u32) -> Option<&Symbol> {
        self.symbols
            .iter()
            .filter(|s| s.is_function && s.address <= address && address < s.address + s.size.max(1))
            .max_by_key(|s| s.address)
    }

    /// The symbol (function or variable) containing `address`.
    pub fn symbol_at(&self, address: u32) -> Option<&Symbol> {
        self.symbols
            .iter()
            .filter(|s| s.address <= address && address < s.address + s.size.max(1))
            .max_by_key(|s| s.address)
    }

    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name)
    }

    pub fn section_size(&self, name: &str) -> u32 {
        self.sections.iter().find(|s| s.0 == name).map(|s| s.2).unwrap_or(0)
    }
}

/// Clean up "a/./b/../c" without touching the file system.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    // Resolve symlinks like /tmp -> /private/tmp when the file exists.
    out.canonicalize().unwrap_or(out)
}
