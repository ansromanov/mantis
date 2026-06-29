//! Git integration for `mantis`.
//!
//! All git functionality is built in — repo status, blame, diff, history, and
//! branch info are provided by shelling out to the `git` CLI. Types and
//! implementations are always compiled.

mod types;
pub use types::*;

mod core;
pub use core::*;
