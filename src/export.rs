use std::{
    collections::{hash_map::RandomState, HashMap},
    fs::{create_dir_all, File},
    io::Write,
};

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::Args;
use itertools::Itertools;

use crate::{
    config::{SyncConfig, SyncedDocument},
    onshape::{
        environment_client,
        models::{Part, TranslationJobWithOutput, TranslationState},
    },
    GlobalOptions,
};

#[derive(Args, Debug)]
pub struct ExportOptions {
    #[arg(long)]
    pub no_clean_paths: bool,
}
impl ExportOptions {
    fn should_clean_paths(&self) -> bool {
        !self.no_clean_paths
    }
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            no_clean_paths: false,
        }
    }
}

pub fn export(
    config: SyncConfig,
    global_options: GlobalOptions,
    options: ExportOptions,
) -> Result<()> {
    // Load the manifest describing what to sync
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

        let parts =
            client.get_studio_parts(&document_id, &workspace_id, &synced_studio.id)?;
        let parts_by_id = HashMap::<String, Part, RandomState>::from_iter(
            parts.iter().map(|p| (p.part_id.clone(), p.clone())),
        );

        let mut to_export = vec![];
        for p in synced_studio.synced_parts.iter() {
            if !parts_by_id.contains_key(&p.id) {
                return Err(anyhow!("Part not found, part_id={}", p.id));
            }
            let part = parts_by_id.get(&p.id).unwrap();
            to_export.push((part.part_id.clone(), p.basename.clone()));
        }
        to_export_by_studio.insert(&synced_studio.id, to_export);
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
            for f in formats.iter() {
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
                        let bytes = client.download_translated_file(&j)?;

                        let mut output_path: Utf8PathBuf =
                            config.format_path(&j.format).unwrap().into();
                        output_path.push(j.output_filename.clone());
                        eprintln!("Writing translation to {}", j.output_filename);
                        let mut output_file = File::create(output_path)?;
                        output_file.write_all(&bytes)?;
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
