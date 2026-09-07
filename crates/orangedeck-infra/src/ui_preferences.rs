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

#[cfg(test)]
mod tests {
    use super::*;
    use orangedeck_domain::{ConversationSlot, Shortcut, UiLanguage};

    #[test]
    fn old_preferences_load_and_new_assignments_persist_without_duplicate_threads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-preferences.toml");
        fs::write(&path, "schema_version = 1\nlanguage = 'english'\n").unwrap();
        let store = UiPreferenceStore::new(path.clone());
        let mut preferences = store.load().unwrap();
        assert_eq!(preferences.language, UiLanguage::English);
        assert!(preferences.notification_sound);
        assert!(
            preferences
                .conversations
                .iter()
                .all(|slot| slot.thread_id.is_empty())
        );
        preferences.conversations[0] = ConversationSlot {
            thread_id: "terminal-a".to_owned(),
            label: "My API".to_owned(),
        };
        preferences.notification_sound = false;
        store.save(&preferences).unwrap();
        assert_eq!(store.load().unwrap(), preferences);
        let saved = fs::read(&path).unwrap();
        preferences.conversations[1] = preferences.conversations[0].clone();
        assert!(store.save(&preferences).is_err());
        assert_eq!(fs::read(path).unwrap(), saved);
    }

    #[test]
    fn preferences_persist_without_touching_pairing_and_reject_unrecognized_actions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-preferences.toml");
        let store = UiPreferenceStore::new(path.clone());
        let token = directory.path().join("ui.token");
        fs::write(&token, "existing private token fixture").unwrap();
        let mut preferences = store.load().unwrap();
        preferences.language = UiLanguage::English;
        preferences.assign(9, Some(Shortcut::OpenEditor));
        store.save(&preferences).unwrap();
        assert_eq!(store.load().unwrap(), preferences);
        assert_eq!(
            fs::read_to_string(token).unwrap(),
            "existing private token fixture"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::write(&path, "language = 'korean'\nshortcuts = ['shell']").unwrap();
        assert!(store.load().is_err());
        assert!(fs::read_to_string(path).unwrap().contains("shell"));
    }

    #[cfg(unix)]
    #[test]
    fn preferences_never_follow_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("other.toml");
        fs::write(&target, "keep me").unwrap();
        let path = directory.path().join("ui-preferences.toml");
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let store = UiPreferenceStore::new(path);
        assert!(store.load().is_err());
        assert!(store.save(&UiPreferences::default()).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "keep me");
    }
}
