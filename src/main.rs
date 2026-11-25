use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{Parser, Subcommand};
use frameo_linux::Unframeo;

#[derive(Debug, Parser)]
#[command(
    name = "frameo",
    version,
    about = "Send photos to Frameo digital photo frames",
    long_about = "A desktop-independent command-line client for Frameo. Protocol operations are currently provided by the unframeo backend."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Pair with a frame using the code displayed on its screen.
    Pair {
        /// The pairing code shown by the Frameo frame.
        code: String,
    },
    /// Upload one or more local photos.
    Upload {
        /// Optional caption shown with the uploaded photo(s).
        #[arg(short, long)]
        caption: Option<String>,
        /// Image files to upload.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
    },
    /// List frames already paired with this client.
    Frames,
    /// Discover Frameo frames on the local network.
    Discover,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let backend = Unframeo::default();

    let status = match cli.command {
        Commands::Pair { code } => backend.pair(&code)?,
        Commands::Upload { caption, files } => backend.upload(&files, caption.as_deref())?,
        Commands::Frames => backend.frames()?,
        Commands::Discover => backend.discover()?,
    };

    Ok(ExitCode::from(status.code().unwrap_or(1) as u8))
}
