//! Parsing compiler and linker messages into structured diagnostics, and
//! adding beginner-friendly hints to the most common errors.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// One problem reported by the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Absolute path of the file, if the message points at one.
    pub file: Option<PathBuf>,
    /// 1-based line and column (0 = unknown).
    pub line: u32,
    pub column: u32,
    pub message: String,
    /// Extra explanation written by GBADE for beginners.
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Diagnostic {
        let mut d = Diagnostic {
            severity: Severity::Error,
            file: None,
            line: 0,
            column: 0,
            message: message.into(),
            hint: None,
        };
        d.hint = hint_for(&d.message);
        d
    }

    /// "main.c:12:5: error: ..." style one-line summary.
    pub fn summary(&self) -> String {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        match &self.file {
            Some(f) => {
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if self.line > 0 {
                    format!("{name}:{}:{}: {sev}: {}", self.line, self.column, self.message)
                } else {
                    format!("{name}: {sev}: {}", self.message)
                }
            }
            None => format!("{sev}: {}", self.message),
        }
    }
}

/// Parse clang's text output (`-fno-color-diagnostics`). Lines look like:
/// `/abs/path/main.c:12:5: error: use of undeclared identifier 'x'`.
pub fn parse_clang(output: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for line in output.lines() {
        if let Some(d) = parse_clang_line(line) {
            // Attach "note:" lines that follow an error as separate notes.
            out.push(d);
        }
    }
    out
}

fn parse_clang_line(line: &str) -> Option<Diagnostic> {
    // clang driver errors: "clang: error: no such file or directory: 'x'"
    if let Some(rest) = line.strip_prefix("clang: error: ") {
        return Some(Diagnostic::error(rest.trim()));
    }
    for (tag, sev) in [(": error: ", Severity::Error), (": fatal error: ", Severity::Error), (": warning: ", Severity::Warning), (": note: ", Severity::Note)] {
        if let Some(pos) = line.find(tag) {
            let loc = &line[..pos];
            let message = line[pos + tag.len()..].trim().to_string();
            let (file, l, c) = parse_location(loc);
            let mut d = Diagnostic { severity: sev, file, line: l, column: c, message, hint: None };
            d.hint = hint_for(&d.message);
            return Some(d);
        }
    }
    None
}

/// Split "path:line:col" (path may itself contain ':' on weird systems).
fn parse_location(loc: &str) -> (Option<PathBuf>, u32, u32) {
    let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
    match parts.as_slice() {
        [col, line, file] if col.parse::<u32>().is_ok() && line.parse::<u32>().is_ok() => {
            (Some(PathBuf::from(file)), line.parse().unwrap(), col.parse().unwrap())
        }
        [line, file] if line.parse::<u32>().is_ok() => (Some(PathBuf::from(file)), line.parse().unwrap(), 0),
        _ if loc.is_empty() => (None, 0, 0),
        _ => (Some(PathBuf::from(loc)), 0, 0),
    }
}

/// Parse ld.lld output. Errors look like:
/// ```text
/// ld.lld: error: undefined symbol: foo
/// >>> referenced by main.c:12 (/abs/src/main.c:12)
/// >>>               /abs/build/obj/main.o:(main)
/// ```
pub fn parse_lld(output: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for line in output.lines() {
        let (sev, rest) = if let Some(r) = line.strip_prefix("ld.lld: error: ") {
            (Severity::Error, r)
        } else if let Some(r) = line.strip_prefix("ld.lld: warning: ") {
            (Severity::Warning, r)
        } else if let Some(r) = line.trim_start().strip_prefix(">>> referenced by ") {
            // Point the previous diagnostic at the referencing source line.
            if let Some(last) = out.last_mut() {
                if last.file.is_none() {
                    if let (Some(open), Some(close)) = (r.find('('), r.rfind(')')) {
                        let (file, l, c) = parse_location(&r[open + 1..close]);
                        last.file = file;
                        last.line = l;
                        last.column = c;
                    }
                }
            }
            continue;
        } else {
            continue;
        };
        let mut d = Diagnostic { severity: sev, file: None, line: 0, column: 0, message: rest.trim().to_string(), hint: None };
        d.hint = hint_for(&d.message);
        out.push(d);
    }
    out
}

/// Friendly explanations for errors beginners hit all the time.
pub fn hint_for(message: &str) -> Option<String> {
    let m = message;
    let hint = if m.starts_with("undefined symbol: main") {
        "Every GBA game needs an `int main(void)` function: it is where your game starts."
    } else if let Some(sym) = m.strip_prefix("undefined symbol: ") {
        return Some(format!(
            "The linker cannot find `{sym}`. Check the spelling, make sure the function is defined in one of \
             your .c files under src/ (not only declared in a header), and that it is not marked `static` in \
             another file."
        ));
    } else if m.starts_with("duplicate symbol: ") {
        "The same function or global variable is defined in two .c files. Define it in one file and use \
         `extern` (for variables) or a prototype (for functions) in a header for the others."
    } else if m.contains("will not fit in region 'iwram'") || m.contains("region 'iwram' overflowed") {
        "IWRAM (the fast 32 KB RAM) is full. Move big arrays to EWRAM with `EWRAM_BSS` / `EWRAM_DATA`, \
         make constant tables `const` (so they stay in ROM), or remove IWRAM_CODE from large functions."
    } else if m.contains("will not fit in region 'ewram'") || m.contains("region 'ewram' overflowed") {
        "EWRAM (256 KB) is full. Make constant data `const` so it stays in ROM instead of RAM."
    } else if m.contains("implicit declaration of function") || m.contains("call to undeclared function") {
        "You are calling a function the compiler has never seen. Add the right #include (e.g. <gba.h>, \
         <stdio.h>, <string.h>) or declare the function before using it."
    } else if m.contains("use of undeclared identifier") {
        "This name does not exist here. Check the spelling (C is case-sensitive), and that the variable is \
         declared before this line and in this scope."
    } else if m.starts_with("expected ';'") {
        "A statement is missing its `;` at the end, often on the line ABOVE the one highlighted."
    } else if m.contains("file not found") {
        "This header cannot be found. GBADE provides <gba.h>, <stdio.h>, <stdlib.h>, <string.h>, <stdint.h>, \
         <stdbool.h>... Your own headers go in include/ or next to your .c files. Generated asset headers \
         are included with #include \"assets.h\"."
    } else if m.contains("incompatible pointer") || m.contains("incompatible integer to pointer") {
        "You are mixing up a pointer and a value (or two different pointer types). On the GBA, memory \
         addresses are cast like `(u16 *)0x06000000`."
    } else if m.contains("control reaches end of non-void function") {
        "This function promises to return a value but can reach its end without a `return`."
    } else {
        return None;
    };
    Some(hint.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clang_error_and_warning() {
        let out = "/p/src/main.c:12:5: error: use of undeclared identifier 'x'\n    x = 1;\n    ^\n\
                   /p/src/main.c:3:9: warning: unused variable 'y' [-Wunused-variable]\n1 error generated.\n";
        let d = parse_clang(out);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].severity, Severity::Error);
        assert_eq!(d[0].file.as_deref(), Some(std::path::Path::new("/p/src/main.c")));
        assert_eq!((d[0].line, d[0].column), (12, 5));
        assert!(d[0].hint.is_some());
        assert_eq!(d[1].severity, Severity::Warning);
        assert_eq!(d[1].line, 3);
        assert_eq!(d[0].summary(), "main.c:12:5: error: use of undeclared identifier 'x'");
    }

    #[test]
    fn parses_fatal_error_and_driver_error() {
        let d = parse_clang("/p/a.c:1:10: fatal error: 'foo.h' file not found\nclang: error: unknown argument: '-bad'\n");
        assert_eq!(d.len(), 2);
        assert!(d[0].hint.as_ref().unwrap().contains("include/"));
        assert!(d[1].file.is_none());
    }

    #[test]
    fn parses_lld_undefined_symbol_with_location() {
        let out = "ld.lld: error: undefined symbol: draw_hero\n>>> referenced by main.c:42 (/p/src/main.c:42)\n>>>               /p/build/obj/main.o:(main)\n";
        let d = parse_lld(out);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message, "undefined symbol: draw_hero");
        assert_eq!(d[0].line, 42);
        assert_eq!(d[0].file.as_deref(), Some(std::path::Path::new("/p/src/main.c")));
        assert!(d[0].hint.as_ref().unwrap().contains("draw_hero"));
    }

    #[test]
    fn missing_main_hint() {
        let d = parse_lld("ld.lld: error: undefined symbol: main\n");
        assert!(d[0].hint.as_ref().unwrap().contains("int main"));
    }

    #[test]
    fn iwram_overflow_hint() {
        let h = hint_for("section '.bss' will not fit in region 'iwram': overflowed by 100 bytes").unwrap();
        assert!(h.contains("EWRAM_BSS"));
    }
}
