//! Shared application logic for the Frameo Linux interfaces.
//!
//! The CLI currently retains a temporary adapter to the upstream unframeo
//! executable. Native protocol primitives and configuration live in modules
//! shared by the CLI and the future GTK application.

pub mod config;
pub mod transport;

// Temporary compatibility adapter. It will be removed when native pairing
// and transfer are implemented.
use std::{
    env,
    ffi::{OsStr, OsString},
    path::Path,
    process::{Command, ExitStatus},
};

use anyhow::{Context, Result, bail};

pub const BACKEND_ENV: &str = "FRAMEO_UNFRAMEO_BIN";

#[derive(Debug, Clone)]
pub struct Unframeo {
    executable: OsString,
}

impl Default for Unframeo {
    fn default() -> Self {
        Self {
            executable: env::var_os(BACKEND_ENV).unwrap_or_else(|| OsString::from("unframeo")),
        }
    }
}

impl Unframeo {
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self { executable: executable.into() }
    }

    pub fn pair(&self, code: &str) -> Result<ExitStatus> {
        self.run([OsString::from("pair"), OsString::from(code)])
    }

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
        self.run(upload_args(files, caption))
    }

    pub fn frames(&self) -> Result<ExitStatus> {
        self.run([OsString::from("frames")])
    }

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
    if let Some(caption) = caption {
        args.push(OsString::from("-caption"));
        args.push(OsString::from(caption));
    }
    args.push(OsString::from("send"));
    args.extend(files.iter().map(|file| file.as_ref().as_os_str().to_os_string()));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_arguments_include_caption_before_send() {
        let files = ["/tmp/one.jpg", "/tmp/two.jpg"];
        let args = upload_args(&files, Some("Summer holiday"));
        let args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["-caption", "Summer holiday", "send", "/tmp/one.jpg", "/tmp/two.jpg"]);
    }

    #[test]
    fn upload_arguments_without_caption() {
        let files = ["/tmp/photo.jpg"];
        let args = upload_args(&files, None);
        let args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["send", "/tmp/photo.jpg"]);
    }
}
