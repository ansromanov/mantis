//! Git integration for `mantis`.
//!
//! Types are always compiled; function implementations are behind the
//! `git-core` feature flag (default on). When `git-core` is disabled the
//! bundled `git-plugin` subprocess provides the same functionality.

mod types;
pub use types::*;

#[cfg(feature = "git-core")]
mod core;
#[cfg(feature = "git-core")]
pub use core::*;
