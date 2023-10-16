use anyhow::Result;
use clap::{Args, ValueEnum};
use convert_case::{Case, Casing};
use indoc::printdoc;

use crate::{
    config::{SyncConfig, SyncedDocument},
    onshape::{environment_client, models::Part},
    GlobalOptions,
};

#[derive(Args, Debug)]
pub struct ShowPartsOptions {
    #[clap(long, short, default_value_t = OutputFormat::default())]
    pub format: OutputFormat,
}
impl Default for ShowPartsOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Friendly,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Friendly,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Friendly
    }
}

impl ToString for OutputFormat {
    fn to_string(&self) -> String {
        match self {
            OutputFormat::Friendly => "friendly",
            OutputFormat::Json => "json",
        }
        .into()
    }
}

pub fn show_parts(
    config: SyncConfig,
    global_options: GlobalOptions,
    options: ShowPartsOptions,
) -> Result<()> {
    // Load the manifest describing what to sync
    let SyncConfig {
        document:
            SyncedDocument {
                id: document_id,
                workspace_id,
            },
        part_studios,
        ..
    } = config;

    let client = environment_client(global_options.proxy_url)?;
    let element_map = client.get_document_elements(&document_id, &workspace_id)?;

    for sync_part_studio in part_studios {
        if !element_map.contains_key(&sync_part_studio.id) {
            panic!("Could not find an part_studio ({})", sync_part_studio.id);
        }

        match options.format {
            OutputFormat::Friendly => {
                println!("PART_STUDIO {}\n", sync_part_studio.display_name);

                let studio_parts = client.get_studio_parts(
                    &document_id,
                    &workspace_id,
                    &sync_part_studio.id,
                )?;
                for part in studio_parts {
                    let Part {
                        ref name,
                        ref part_id,
                        ..
                    } = part;
                    let basename = name.to_case(Case::Snake);
                    println!("PART {name}");
                    println!("{:#?}", part);
                    printdoc! {"
                        offshape.toml entry:
                        # {name}
                        {{ id = \"{part_id}\", basename = \"{basename}\"}},

                    "};
                }
            }
            OutputFormat::Json => {
                let json = client.get_studio_parts_json(
                    &document_id,
                    &workspace_id,
                    &sync_part_studio.id,
                )?;
                println!("{}", json);
            }
        }
    }

    Ok(())
}
