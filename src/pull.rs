use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::Write,
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::Args;
use convert_case::{Case, Casing};
use itertools::Itertools;

use crate::{
    config::{SyncConfig, SyncedDocument},
    onshape::{
        environment_client,
        models::{
            ExportAction, ExportFileFormat, TranslationJobWithOutput, TranslationState,
        },
    },
    GlobalOptions,
};

#[derive(Args, Debug)]
pub struct PullOptions {
    #[arg(long)]
    pub no_clean_paths: bool,
    /// If `true` (the default), files will be stripped of timestamps relating to their
    /// export time, or the time they were written to disk.
    #[arg(long, default_value_t = true)]
    pub strip_indeterminism: bool,
}
impl PullOptions {
    fn should_clean_paths(&self) -> bool {
        !self.no_clean_paths
    }
}
impl Default for PullOptions {
    fn default() -> Self {
        Self {
            no_clean_paths: false,
            strip_indeterminism: true,
        }
    }
}

pub fn export(
    config: SyncConfig,
    global_options: GlobalOptions,
    options: PullOptions,
) -> Result<()> {
    // Load the manifest describing what to pull
    let SyncConfig {
        document:
            SyncedDocument {
                id: ref document_id,
                ref workspace_id,
            },
        ..
    } = config;
    let part_studios = config.part_studios.clone();
    let formats = config.export_formats();

    let client = environment_client(global_options.proxy_url)?;

    // Validate that the part studios and parts exist
    let element_map = client.get_document_elements(&document_id, &workspace_id)?;
    let mut to_export_by_studio = HashMap::new();
    for synced_studio in part_studios.iter() {
        if !element_map.contains_key(&synced_studio.id) {
            return Err(anyhow!(
                "Could not find a part studio ({})",
                synced_studio.id
            ));
        }

        let studio_parts: Vec<(String, String)> = client
            .get_studio_parts(&document_id, &workspace_id, &synced_studio.id)?
            .iter()
            .map(|p| {
                let basename = p.name.to_case(Case::Snake);
                (p.part_id.clone(), basename)
            })
            .collect();
        to_export_by_studio.insert(&synced_studio.id, studio_parts);
    }

    // Create/clean output directories
    for f in formats.iter() {
        let path = config.format_path(f);
        if let Some(path) = path {
            create_dir_all(&(*path.clone()))?;
            if options.should_clean_paths() {
                clean_path(&path, &f.extension());
            }
        }
    }

    // Begin translating the parts
    let mut active_jobs = vec![];
    for part_studio in part_studios.iter() {
        let to_sync = to_export_by_studio.get(&part_studio.id).unwrap();
        for (part_id, basename) in to_sync {
            // Begin translations for the formats that require them
            for f in formats
                .iter()
                .filter(|f| f.export_action() == ExportAction::Translate)
            {
                eprintln!("Exporting {}.{}", basename, f.extension());
                active_jobs.push(client.begin_translation(
                    &f,
                    &document_id,
                    &workspace_id,
                    &part_studio.id,
                    &part_id,
                    &basename,
                )?);
            }

            for f in formats
                .iter()
                .copied()
                .filter(|f| f.export_action() == ExportAction::Direct)
            {
                eprintln!("Exporting {}.{}", basename, f.extension());
                if *f == ExportFileFormat::Stl {
                    let stl_contents = client.get_part_stl(
                        &document_id,
                        &workspace_id,
                        &part_studio.id,
                        &part_id,
                    )?;

                    // TODO(shyndman): Figure out how to merge the STL file writes with
                    // the 3mf and step files
                    let mut output_path: Utf8PathBuf = config.format_path(f).unwrap().into();
                    output_path.push(format!("{basename}.{ext}", ext = f.extension()));
                    write_output_file(
                        output_path,
                        stl_contents.as_bytes(),
                        options.strip_indeterminism,
                    )?;
                }
            }
        }
    }

    // Check on the translation jobs repeatedly
    while !active_jobs.is_empty() {
        let mut next: Vec<TranslationJobWithOutput> = vec![];
        for (group, jobs) in active_jobs
            .iter()
            .map(|j| client.check_translation(j).unwrap())
            .group_by(|j| j.request_state)
            .into_iter()
        {
            match group {
                TranslationState::Active => {
                    next = jobs.collect();
                }
                TranslationState::Done => {
                    for j in jobs {
                        let bytes = client
                            .download_translated_file(&j, options.strip_indeterminism)?;

                        let mut output_path: Utf8PathBuf =
                            config.format_path(&j.format).unwrap().into();
                        output_path.push(j.output_filename.clone());
                        eprintln!("Writing translation to {}", j.output_filename);
                        write_output_file(output_path, &bytes, options.strip_indeterminism)?;
                    }
                }
                TranslationState::Failed => {
                    for j in jobs {
                        let failure_reason = &(*j)
                            .failure_reason
                            .clone()
                            .unwrap_or("Unknown reason".into());
                        eprintln!("Translation failed: {}", failure_reason);
                    }
                }
            }
        }
        active_jobs = next;
    }

    Ok(())
}

fn write_output_file(
    output_path: Utf8PathBuf,
    bytes: &[u8],
    strip_timestamps: bool,
) -> anyhow::Result<()> {
    let mut f = File::create(output_path)?;
    f.write(bytes)?;
    if strip_timestamps {
        f.set_modified(SystemTime::UNIX_EPOCH)?;
    }
    f.flush()?;
    Ok(())
}

fn clean_path(path: &camino::Utf8Path, ext: &str) {
    eprintln!("Cleaning {}", path);

    let entries = std::fs::read_dir(path).expect("Could not read path");
    for res in entries {
        let entry = match res {
            Ok(entry) => entry,
            Err(e) => panic!("{}", e),
        };

        let name = entry.file_name().into_string().expect("");
        if name.ends_with(ext) {
            std::fs::remove_file(entry.path()).expect("Could not delete file");
        }
    }
}
