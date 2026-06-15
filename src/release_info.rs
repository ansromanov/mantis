//! Compile-time "what's new" release metadata.
//!
//! `release-info.toml` (at the crate root) is embedded at build time via
//! `include_str!` and lazily parsed into a `ReleaseInfo` - the version, date,
//! changelog blurb, and release URL. The `RELEASE` static exposes it as an
//! `Option` so a missing or malformed file simply disables the feature instead
//! of breaking the build or the app. The About popup reads this to show the
//! current release notes and to offer opening the linked release page in the
//! user's browser. The TOML file is updated by the release tooling, not edited
//! by hand.

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub date: String,
    pub whats_new: String,
    pub release_url: String,
}

pub static RELEASE: LazyLock<Option<ReleaseInfo>> =
    LazyLock::new(|| toml::from_str(include_str!("../release-info.toml")).ok());
