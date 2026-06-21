//! Crate root for the `tv` terminal file viewer library.
//!
//! Declares every top-level module; the binary (`main.rs`) and the integration
//! tests both depend on this crate. The module layout mirrors the runtime
//! architecture: `app` owns the central state machine and input handling, `ui`
//! owns ratatui rendering, and `config` parses `tv.toml`. The remaining leaf
//! modules (`tree`, `git`, `highlight`, `markdown`, `search`, `diff`,
//! `selection`, `theme`, `virtual_file`, `yaml_fold`, `file`, `command_palette`,
//! `release_info`, `ansi`) each provide one focused capability consumed by
//! `app`/`ui`. Keep this list in sync with the directory structure documented
//! in AGENTS.md.

pub mod ansi;
pub mod app;
pub mod command_palette;
pub mod config;
pub mod diff;
pub mod file;
pub mod git;
pub mod highlight;
#[cfg(feature = "markdown-core")]
pub mod markdown;
pub mod plugin;
pub mod release_info;
pub mod search;
pub mod selection;
pub mod theme;
pub mod tree;
pub mod ui;
pub mod virtual_file;
pub mod yaml_fold;
