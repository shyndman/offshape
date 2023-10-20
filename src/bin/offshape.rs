use std::process::exit;

use anyhow::Result;
use camino::*;
use clap::{Parser, Subcommand};
use offshape::{
    export, load_config, show_parts, GlobalOptions, PullOptions, ShowPartsOptions,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long = "config", value_name = "offshape.toml")]
    config_path: Option<Utf8PathBuf>,

    #[command(flatten)]
    global_options: GlobalOptions,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Displays all OnShape parts found in the offshape.toml's [[part_studio]] tabs
    ShowParts(ShowPartsOptions),
    /// Pulls the latest CAD files (3mf, STL, STEP, etc) from OnShape, and write them to
    /// the paths found in offshape.toml
    Pull(PullOptions),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config_path.unwrap_or("offshape.toml".into());
    if !config_path.exists() {
        eprintln!("offshape.toml not found");
        exit(1);
    }

    let config = load_config(&config_path)?;
    match cli.command {
        Commands::ShowParts(options) => show_parts(config, cli.global_options, options),
        Commands::Pull(options) => export(config, cli.global_options, options),
    }
}
