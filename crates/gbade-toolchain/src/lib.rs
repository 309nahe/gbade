//! # gbade-toolchain
//!
//! The GBADE build system. It turns a project folder (C sources + PNG assets)
//! into a Game Boy Advance ROM using the bundled LLVM clang and lld, with no
//! setup at all for the user.
//!
//! ```no_run
//! use gbade_toolchain::{Project, Toolchain, BuildOptions, build};
//! let tc = Toolchain::locate().unwrap();
//! let project = Project::open(std::path::Path::new("my-game")).unwrap();
//! let out = build(&project, &tc, &BuildOptions::default());
//! for d in &out.diagnostics { println!("{}", d.summary()); }
//! assert!(out.success);
//! ```

pub mod assets;
pub mod builder;
pub mod debuginfo;
pub mod diagnostics;
mod nintendo_logo;
pub mod project;
pub mod rom;
pub mod toolchain;

pub use builder::{BuildOptions, BuildOutput, MemoryUsage, build, default_cache_dir};
pub use debuginfo::DebugInfo;
pub use diagnostics::{Diagnostic, Severity};
pub use project::{Project, ProjectConfig};
pub use toolchain::Toolchain;
