use std::{ops::Deref, slice::Iter};

use camino::Utf8PathBuf;
use reqwest::Url;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize)]
pub struct Document {
    pub id: String,
    pub name: String,
    #[serde(rename = "defaultWorkspace")]
    pub default_workspace: Workspace,
}

#[derive(Debug, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub href: Url,
}

#[derive(Debug, Deserialize)]
pub struct DocumentElement {
    pub id: String,
    pub name: String,
    #[serde(rename = "filename")]
    pub file_name: Option<String>,
    #[serde(rename = "elementType")]
    pub element_type: TabElementType,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Part {
    pub name: String,
    #[serde(rename = "partId")]
    pub part_id: String,
    #[serde(rename = "elementId")]
    pub element_id: String,
    #[serde(rename = "microversionId")]
    pub microversion_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationRequest {
    #[serde(rename = "formatName")]
    pub format: ExportFileFormat,
    #[serde(rename = "partIds")]
    pub part_ids: String,
    #[serde(rename = "destinationName")]
    pub destination_name: String,
    #[serde(rename = "storeInDocument")]
    pub store_in_document: bool,
    pub configuration: String,
    #[serde(rename = "angularTolerance")]
    pub angular_tolerance: f32,
    #[serde(rename = "distanceTolerance")]
    pub distance_tolerance: f32,
    pub resolution: TranslationResolution,
    #[serde(rename = "maximumChordLength")]
    pub maximum_chord_length: f32,
    #[serde(rename = "specifyUnits")]
    pub specify_units: bool,
    pub units: TranslationUnit,

    #[serde(rename = "imageWidth")]
    pub image_width: u32,
    #[serde(rename = "imageHeight")]
    pub image_height: u32,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TranslationUnit {
    #[serde(rename = "millimeter")]
    Millimeters,
    #[serde(rename = "centimeter")]
    Centimeters,
    #[serde(rename = "meter")]
    Meters,
    #[serde(rename = "inch")]
    Inches,
    #[serde(rename = "foot")]
    Feet,
    #[serde(rename = "yard")]
    Yards,
}
#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum ExportFileFormat {
    #[serde(rename = "3MF")]
    ThreeMF,
    #[serde(rename = "STEP")]
    Step,
    #[serde(rename = "STL")]
    Stl,
}
impl ExportFileFormat {
    pub fn iter() -> Iter<'static, ExportFileFormat> {
        static FORMATS: [ExportFileFormat; 3] = [
            ExportFileFormat::ThreeMF,
            ExportFileFormat::Step,
            ExportFileFormat::Stl,
        ];
        FORMATS.iter()
    }

    pub fn extension(&self) -> String {
        match self {
            ExportFileFormat::ThreeMF => "3mf",
            ExportFileFormat::Step => "step",
            ExportFileFormat::Stl => "stl",
        }
        .into()
    }

    pub fn export_action(&self) -> ExportAction {
        match self {
            ExportFileFormat::ThreeMF => ExportAction::Translate,
            ExportFileFormat::Step => ExportAction::Translate,
            ExportFileFormat::Stl => ExportAction::Direct,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportAction {
    Translate,
    Direct,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TranslationResolution {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "coarse")]
    Coarse,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "fine")]
    Fine,
    #[serde(rename = "very_fine")]
    VeryFine,
    #[serde(rename = "curvature_visualization")]
    CurvatureVisualization,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug)]
pub struct TranslationJobWithOutput {
    pub job: TranslationJob,
    pub output_filename: Utf8PathBuf,
    pub format: ExportFileFormat,
}
impl Deref for TranslationJobWithOutput {
    type Target = TranslationJob;

    fn deref(&self) -> &Self::Target {
        &self.job
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TranslationJob {
    pub name: String,
    #[serde(rename = "href")]
    pub url: Url,
    #[serde(rename = "requestState")]
    pub request_state: TranslationState,
    #[serde(rename = "failureReason")]
    pub failure_reason: Option<String>,
    #[serde(rename = "documentId")]
    pub document_id: String,
    #[serde(rename = "resultExternalDataIds")]
    pub result_external_data_ids: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum TranslationState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DONE")]
    Done,
    #[serde(rename = "FAILED")]
    Failed,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum TabElementType {
    #[serde(rename = "APPLICATION")]
    Application,
    #[serde(rename = "ASSEMBLY")]
    Assembly,
    #[serde(rename = "BILLOFMATERIALS")]
    BillOfMaterials,
    #[serde(rename = "BLOB")]
    Blob,
    #[serde(rename = "DRAWING")]
    Drawing,
    #[serde(rename = "FEATURESTUDIO")]
    FeatureStudio,
    #[serde(rename = "PARTSTUDIO")]
    PartStudio,
    #[serde(rename = "PUBLICATIONITEM")]
    PublicationItem,
    #[serde(rename = "TABLE")]
    Table,
    #[serde(rename = "VARIABLESTUDIO")]
    VariableStudio,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum InstanceType {
    Assembly,
    Feature,
    Part,
    Unknown,
}
