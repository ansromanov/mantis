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
