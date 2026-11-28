use std::{net::SocketAddr, path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use frameo_linux::{config::Config, frameo::client::Client, transport::sdg::local::dial_local};

#[derive(Debug, Parser)]
#[command(
    name = "frameo",
    version,
    about = "Send photos to Frameo digital photo frames",
    long_about = "Framebridge native Rust support is experimental."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Experimental fully native upload to a *known* local frame TCP address.
    /// Requires an existing paired identity; discovery and relay are not used.
    NativeLocalSend {
        /// Frame's TCP IP:port obtained by trusted local service discovery.
        #[arg(long)]
        address: SocketAddr,
        /// Select an existing paired frame; defaults to configured frame.
        #[arg(long)]
        frame: Option<String>,
        /// Optional override for the XDG-compatible pairing configuration.
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(short, long, default_value = "")]
        caption: String,
        /// Fit entire image instead of cropping to fill the screen.
        #[arg(long)]
        fit: bool,
        /// Per-operation I/O timeout in seconds.
        #[arg(long, default_value_t = 20)]
        timeout_seconds: u64,
        /// Local photos to send.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
    },
    /// List locally paired frames.
    Frames,
    /// Generate a new native client identity.
    Init,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    if let Commands::NativeLocalSend {
        address,
        frame,
        config,
        caption,
        fit,
        timeout_seconds,
        files,
    } = &cli.command
    {
        anyhow::ensure!(*timeout_seconds > 0, "timeout must be greater than zero");
        for file in files {
            anyhow::ensure!(file.is_file(), "not a file: {}", file.display());
        }
        let stored = Config::load(config.as_deref())?;
        let identity = stored.identity()?;
        let (_, peer) = stored.resolve_frame(frame.as_deref())?;
        let stream = dial_local(
            *address,
            &identity,
            peer,
            Duration::from_secs(*timeout_seconds),
        )
        .context("native local connection failed; verify the frame address and pairing")?;
        let mut client = Client::new(stream, stored.client_name);
        for file in files {
            let media_id = client
                .send_photo_file(file, caption, *fit)
                .with_context(|| format!("native upload {}", file.display()))?;
            println!("Sent {} (media ID {})", file.display(), media_id);
        }
        return Ok(ExitCode::SUCCESS);
    }

    match cli.command {
        Commands::Frames => {
            let config = Config::load(None)?;
            for (name, frame) in config.frames {
                println!("{} {}", name, frame.peer_id);
            }
        }
        Commands::Init => {
            let config = Config::create_for_pairing(None)?;
            println!("Created identity at {}", config.path().display());
        }
        Commands::NativeLocalSend { .. } => unreachable!("handled above"),
    }
    Ok(ExitCode::SUCCESS)
}
