//! Crate root for the `mantis` terminal file viewer library.
//!
//! Declares every top-level module; the binary (`main.rs`) and the integration
//! tests both depend on this crate. The module layout mirrors the runtime
//! architecture: `app` owns the central state machine and input handling, `ui`
//! owns ratatui rendering, and `config` parses `mantis.toml`. The remaining leaf
//! modules (`tree`, `git`, `highlight`, `search`, `diff`,
//! `selection`, `session`, `theme`, `virtual_file`, `fold`, `yaml_fold`, `file`,
//! `command_palette`, `release_info`, `ansi`, `actions`, `pager`) each provide
//! one focused capability consumed by `app`/`ui`. Keep this list in sync with
//! the directory structure documented in AGENTS.md. `actions` is the
//! canonical action registry that `command_palette`, `config::keymap`, and
//! `ui::popups::help` all derive from. `pager` handles piped-stdin ingestion
//! for pager mode (`git diff | mantis`).

pub mod actions;
pub mod ansi;
pub mod app;
pub mod command_palette;
pub mod command_usage;
pub mod config;
pub mod diff;
pub mod event_source;
pub mod file;
pub mod fold;
pub mod git;
pub mod highlight;
pub mod list_picker;
pub mod pager;
pub mod plugin;
pub mod release_info;
pub mod search;
pub mod selection;
pub mod session;
pub mod theme;
pub mod tree;
pub mod ui;
pub mod virtual_file;
pub mod yaml_fold;
