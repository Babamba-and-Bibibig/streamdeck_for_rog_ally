//! Private, bounded, atomic UI preferences; never writes pairing or Codex settings.
use orangedeck_domain::UiPreferences;
use std::{fs, io::Read, path::PathBuf};

pub struct UiPreferenceStore {
    path: PathBuf,
}

impl UiPreferenceStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<UiPreferences, String> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let file = match options.open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UiPreferences::default());
            }
            Err(_) => return Err("Cannot read UI preferences".to_owned()),
        };
        let metadata = file
            .metadata()
            .map_err(|_| "Cannot inspect UI preferences")?;
        if !metadata.is_file() || metadata.len() > 16_384 {
            return Err("Invalid UI preferences file".to_owned());
        }
        let mut contents = String::new();
        file.take(16_385)
            .read_to_string(&mut contents)
            .map_err(|_| "Cannot read UI preferences")?;
        if contents.len() > 16_384 {
            return Err("UI preferences are too large".to_owned());
        }
        let preferences: UiPreferences =
            toml::from_str(&contents).map_err(|_| "Invalid UI preferences")?;
        if preferences.schema_version != 1 {
            return Err("Unsupported UI preferences version".to_owned());
        }
        if !preferences.valid_conversations() {
            return Err("Invalid conversation assignments".to_owned());
        }
        Ok(preferences)
    }

    pub fn save(&self, preferences: &UiPreferences) -> Result<(), String> {
        if !preferences.valid_conversations() {
            return Err("Invalid conversation assignments".to_owned());
        }
        if preferences.schema_version != 1 {
            return Err("Unsupported UI preferences version".to_owned());
        }
        // Refuse an existing link/device rather than replacing it. The atomic writer
        // creates a new 0600 file in this same configuration directory.
        crate::check_setup_destinations(&[&self.path], true)
            .map_err(|_| "Cannot save UI preferences")?;
        crate::write_toml_secure(&self.path, preferences)
            .map_err(|_| "Cannot save UI preferences".to_owned())
    }
}
