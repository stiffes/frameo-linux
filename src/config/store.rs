//! XDG-aware configuration storage, compatible with unframeo's JSON schema.

use std::{
    collections::BTreeMap,
    env,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::transport::sdg::{Identity, Key};

/// A paired Frameo device. Its peer ID is the frame's long-term SDG public key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrameRecord {
    pub peer_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired_at: Option<String>,
}

/// Persistent state. The private key is the identity the frame paired with.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub private_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_frame: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frames: BTreeMap<String, FrameRecord>,
    #[serde(skip)]
    path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            private_key: String::new(),
            client_name: String::new(),
            default_frame: String::new(),
            frames: BTreeMap::new(),
            path: PathBuf::new(),
        }
    }
}

impl Config {
    /// Resolve the config path, preserving compatibility with existing
    /// unframeo installations and their paired identity.
    pub fn default_path() -> Result<PathBuf> {
        if let Some(path) = env::var_os("UNFRAMEO_CONFIG").filter(|s| !s.is_empty()) {
            return Ok(PathBuf::from(path));
        }
        if let Some(path) = env::var_os("FRAMEO_CONFIG").filter(|s| !s.is_empty()) {
            return Ok(PathBuf::from(path));
        }
        let dirs = BaseDirs::new().context("cannot determine the user's XDG config directory")?;
        Ok(dirs.config_dir().join("unframeo").join("config.json"))
    }

    /// Load an existing config. This function never creates or rotates an
    /// identity implicitly: only the explicit pairing setup may create one.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => Self::default_path()?,
        };
        let data = fs::read(&path).with_context(|| {
            format!(
                "cannot read configuration at {}; if this is a first run, create an identity as part of pairing",
                path.display()
            )
        })?;
        let mut config: Self = serde_json::from_slice(&data)
            .with_context(|| format!("configuration at {} is not valid JSON", path.display()))?;
        config.path = path;
        config.validate()?;
        Ok(config)
    }

    /// Create a new identity for the pairing flow. Existing config is never
    /// overwritten, because doing so would silently orphan every paired frame.
    pub fn create_for_pairing(path: Option<&Path>) -> Result<Self> {
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => Self::default_path()?,
        };
        if path.exists() {
            bail!(
                "configuration already exists at {}; refusing to replace its identity",
                path.display()
            );
        }
        let identity = Identity::generate();
        let config = Self {
            private_key: identity.private_key().to_hex(),
            client_name: default_client_name(),
            default_frame: String::new(),
            frames: BTreeMap::new(),
            path,
        };
        config.save()?;
        Ok(config)
    }

    /// Reconstruct the persisted SDG identity.
    pub fn identity(&self) -> Result<Identity> {
        let private = Key::from_hex(&self.private_key)
            .context("configuration contains an invalid private key")?;
        Ok(Identity::from_private_key(private))
    }

    /// Return the path this config was loaded from or created at.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Save atomically in the same directory, with owner-only file permissions.
    pub fn save(&self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            bail!("configuration has no file path");
        }
        let parent = self.path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create config directory {}", parent.display()))?;

        let mut data = serde_json::to_vec_pretty(self).context("cannot serialize configuration")?;
        data.push(b'\n');

        let mut temporary = NamedTempFile::new_in(parent)
            .with_context(|| format!("cannot create temporary config file in {}", parent.display()))?;
        temporary.write_all(&data).context("cannot write temporary config file")?;
        temporary.as_file().sync_all().context("cannot sync temporary config file")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o600))
                .context("cannot restrict config file permissions")?;
        }

        temporary
            .persist(&self.path)
            .map_err(|error| error.error)
            .with_context(|| format!("cannot atomically replace {}", self.path.display()))?;
        Ok(())
    }

    /// Record a successful pairing. Re-pairing the same peer updates its name
    /// rather than leaving duplicate entries.
    pub fn add_frame(&mut self, name: impl Into<String>, peer_id: Key) -> Result<String> {
        let mut name = name.into();
        if name.trim().is_empty() {
            name = format!("frame{}", self.frames.len() + 1);
        }

        let peer = peer_id.to_hex();
        let duplicate_name = self
            .frames
            .iter()
            .find_map(|(existing_name, frame)| {
                (frame.peer_id == peer && existing_name != &name).then(|| existing_name.clone())
            });
        if let Some(old_name) = duplicate_name {
            self.frames.remove(&old_name);
            if self.default_frame == old_name {
                self.default_frame = name.clone();
            }
        }

        self.frames.insert(
            name.clone(),
            FrameRecord {
                peer_id: peer,
                name: name.clone(),
                paired_at: None,
            },
        );
        if self.default_frame.is_empty() {
            self.default_frame = name.clone();
        }
        self.save()?;
        Ok(name)
    }

    /// Resolve a named frame or the configured default.
    pub fn resolve_frame(&self, name: Option<&str>) -> Result<(String, Key)> {
        let selected = name
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.default_frame);
        if selected.is_empty() {
            bail!("no frame is paired; pair a frame before using this operation");
        }
        let frame = self
            .frames
            .get(selected)
            .with_context(|| format!("no paired frame named '{selected}'"))?;
        let peer = Key::from_hex(&frame.peer_id)
            .with_context(|| format!("paired frame '{selected}' has an invalid peer ID"))?;
        Ok((selected.to_owned(), peer))
    }

    /// Remove a pairing from local configuration.
    pub fn remove_frame(&mut self, name: &str) -> Result<()> {
        if self.frames.remove(name).is_none() {
            bail!("no paired frame named '{name}'");
        }
        if self.default_frame == name {
            self.default_frame = self.frames.keys().next().cloned().unwrap_or_default();
        }
        self.save()
    }

    /// Validate key material before any caller uses this configuration.
    fn validate(&self) -> Result<()> {
        self.identity()?;
        for (name, frame) in &self.frames {
            Key::from_hex(&frame.peer_id)
                .with_context(|| format!("paired frame '{name}' has an invalid peer ID"))?;
        }
        if !self.default_frame.is_empty() && !self.frames.contains_key(&self.default_frame) {
            bail!(
                "default frame '{}' does not exist in the paired-frame list",
                self.default_frame
            );
        }
        Ok(())
    }
}

fn default_client_name() -> String {
    let user = env::var("USER").unwrap_or_else(|_| "frameo-linux".to_owned());
    let host = env::var("HOSTNAME").unwrap_or_default();
    if host.is_empty() {
        user
    } else {
        format!("{user}@{host}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_and_reloads_an_identity() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("unframeo").join("config.json");
        let config = Config::create_for_pairing(Some(&path)).unwrap();
        let first = config.identity().unwrap();
        let loaded = Config::load(Some(&path)).unwrap();
        let second = loaded.identity().unwrap();

        assert_eq!(first.private_key(), second.private_key());
        assert_eq!(first.public_key(), second.public_key());
        assert!(Config::create_for_pairing(Some(&path)).is_err());
    }

    #[test]
    fn preserves_unframeo_json_schema_and_peer_ids() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut config = Config::create_for_pairing(Some(&path)).unwrap();
        let peer = Key::from_bytes([0x42; 32]);
        config.add_frame("living-room", peer).unwrap();

        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"private_key\""));
        assert!(json.contains("\"default_frame\": \"living-room\""));
        assert!(json.contains("\"peer_id\""));

        let loaded = Config::load(Some(&path)).unwrap();
        assert_eq!(loaded.resolve_frame(None).unwrap(), ("living-room".to_owned(), peer));
    }

    #[test]
    fn missing_config_is_not_created_implicitly() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");
        assert!(Config::load(Some(&path)).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn rejects_corrupt_private_key_and_peer_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, r#"{"private_key":"abcd"}"#).unwrap();
        assert!(Config::load(Some(&path)).is_err());

        let path2 = dir.path().join("bad-peer.json");
        fs::write(
            &path2,
            r#"{"private_key":"77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a","frames":{"broken":{"peer_id":"abcd"}}}"#,
        )
        .unwrap();
        assert!(Config::load(Some(&path2)).is_err());
    }

    #[test]
    fn re_pairing_a_peer_updates_its_name_and_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut config = Config::create_for_pairing(Some(&path)).unwrap();
        let peer = Key::from_bytes([0x11; 32]);
        config.add_frame("first", peer).unwrap();
        config.add_frame("renamed", peer).unwrap();

        assert!(!config.frames.contains_key("first"));
        assert_eq!(config.default_frame, "renamed");
        assert_eq!(config.frames.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn config_file_is_owner_read_write_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        Config::create_for_pairing(Some(&path)).unwrap();
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
