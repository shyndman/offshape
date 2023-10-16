#![feature(let_chains)]

mod config;
mod export;
#[allow(dead_code)]
mod onshape;
mod show;

use std::fs;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};

use crate::config::SyncConfig;
pub use crate::{
    config::GlobalOptions,
    export::{export, ExportOptions},
    show::{show_parts, OutputFormat, ShowPartsOptions},
};

pub fn load_config(config_path: &Utf8Path) -> Result<SyncConfig> {
    let config_path = config_path.canonicalize_utf8()?;
    let config_dir: Utf8PathBuf = config_path.parent().unwrap().into();
    let mut config: SyncConfig = toml::from_str(&fs::read_to_string(config_path)?)?;

    if let Some(three_mf_path) = config.three_mf_path {
        config.three_mf_path = Some({
            let mut p = config_dir.clone();
            p.push(three_mf_path);
            p.into()
        });
    }

    if let Some(step_path) = config.step_path {
        config.step_path = Some({
            let mut p = config_dir.clone();
            p.push(step_path);
            p.into()
        });
    }

    if let Some(stl_path) = config.stl_path {
        config.stl_path = Some({
            let mut p = config_dir.clone();
            p.push(stl_path);
            p.into()
        });
    }

    Ok(config)
}
