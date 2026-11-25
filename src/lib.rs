//! Shared application logic for the Frameo Linux interfaces.
//!
//! The first backend delegates protocol operations to the independently
//! maintained `unframeo` command. Keeping that boundary here lets the CLI
//! and a future GTK frontend share the same integration.

use std::{
    env,
    ffi::{OsStr, OsString},
    path::Path,
    process::{Command, ExitStatus},
};

use anyhow::{Context, Result, bail};

/// Name of the environment variable used to select the unframeo executable.
pub const BACKEND_ENV: &str = "FRAMEO_UNFRAMEO_BIN";

/// A process adapter for the unframeo Frameo-protocol implementation.
#[derive(Debug, Clone)]
pub struct Unframeo {
    executable: OsString,
}

impl Default for Unframeo {
    fn default() -> Self {
        Self {
            executable: env::var_os(BACKEND_ENV)
                .unwrap_or_else(|| OsString::from("unframeo")),
        }
    }
}

impl Unframeo {
    /// Create a backend using a specific executable (useful for tests/custom installs).
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Pair this client with the frame displaying the supplied pairing code.
    pub fn pair(&self, code: &str) -> Result<ExitStatus> {
        self.run([OsString::from("pair"), OsString::from(code)])
    }

    /// Send one or more local image files to the selected frame.
    pub fn upload(&self, files: &[impl AsRef<Path>], caption: Option<&str>) -> Result<ExitStatus> {
        if files.is_empty() {
            bail!("provide at least one photo to upload");
        }

        for file in files {
            let path = file.as_ref();
            if !path.is_file() {
                bail!("not a regular file: {}", path.display());
            }
        }

        let args = upload_args(files, caption);
        self.run(args)
    }

    /// List frames already paired with this client.
    pub fn frames(&self) -> Result<ExitStatus> {
        self.run([OsString::from("frames")])
    }

    /// Discover Frameo devices on the local network.
    pub fn discover(&self) -> Result<ExitStatus> {
        self.run([OsString::from("discover")])
    }

    fn run<I, S>(&self, args: I) -> Result<ExitStatus>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Command::new(&self.executable)
            .args(args)
            .status()
            .with_context(|| {
                format!(
                    "could not start '{}'; install unframeo or set {BACKEND_ENV}",
                    self.executable.to_string_lossy()
                )
            })
    }
}

fn upload_args(files: &[impl AsRef<Path>], caption: Option<&str>) -> Vec<OsString> {
    let mut args = Vec::with_capacity(files.len() + usize::from(caption.is_some()) * 2 + 1);

    // unframeo accepts command options before the subcommand.
    if let Some(caption) = caption {
        args.push(OsString::from("-caption"));
        args.push(OsString::from(caption));
    }

    args.push(OsString::from("send"));
    args.extend(
        files
            .iter()
            .map(|file| file.as_ref().as_os_str().to_os_string()),
    );
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_arguments_include_caption_before_send() {
        let files = ["/tmp/one.jpg", "/tmp/two.jpg"];
        let args = upload_args(&files, Some("Summer holiday"));
        let args: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            args,
            ["-caption", "Summer holiday", "send", "/tmp/one.jpg", "/tmp/two.jpg"]
        );
    }

    #[test]
    fn upload_arguments_without_caption() {
        let files = ["/tmp/photo.jpg"];
        let args = upload_args(&files, None);
        let args: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(args, ["send", "/tmp/photo.jpg"]);
    }
}
