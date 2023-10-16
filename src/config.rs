use camino::Utf8Path;
use clap::Args;
use serde::Deserialize;
use url::Url;

use crate::onshape::models::TranslationFormat;

#[derive(Args, Clone, Debug)]
pub struct GlobalOptions {
    #[arg(short, long = "proxy", value_name = "PROXY_URL")]
    pub proxy_url: Option<Url>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SyncConfig {
    #[serde(rename = "3mf_path")]
    pub three_mf_path: Option<Box<Utf8Path>>,
    pub step_path: Option<Box<Utf8Path>>,
    pub stl_path: Option<Box<Utf8Path>>,

    pub document: SyncedDocument,
    #[serde(rename = "part_studio")]
    pub part_studios: Vec<SyncedPartStudio>,
}
impl SyncConfig {
    pub fn export_formats(&self) -> Vec<&TranslationFormat> {
        TranslationFormat::iter()
            .filter(|f| match **f {
                TranslationFormat::ThreeMF => self.three_mf_path.as_deref().is_some(),
                TranslationFormat::Step => self.step_path.as_deref().is_some(),
                TranslationFormat::Stl => self.stl_path.as_deref().is_some(),
            })
            .collect()
    }

    pub fn format_path(&self, format: &TranslationFormat) -> Option<Box<Utf8Path>> {
        match format {
            TranslationFormat::ThreeMF => self.three_mf_path.clone(),
            TranslationFormat::Step => self.step_path.clone(),
            TranslationFormat::Stl => self.stl_path.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ExportFormat {
    pub format: TranslationFormat,
    pub path: Box<Utf8Path>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SyncedDocument {
    pub id: String,
    pub workspace_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SyncedPartStudio {
    pub display_name: String,
    pub id: String,
    pub synced_parts: Vec<SyncedPart>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SyncedPart {
    pub id: String,
    pub basename: String,
}
