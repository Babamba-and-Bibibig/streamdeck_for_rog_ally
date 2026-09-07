//! User-selectable presentation preferences and the finite shortcut catalog.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    #[default]
    Korean,
    English,
}

impl UiLanguage {
    pub const fn text<'a>(self, korean: &'a str, english: &'a str) -> &'a str {
        match self {
            Self::Korean => korean,
            Self::English => english,
        }
    }
}

/// Approval decisions are deliberately absent: the first two keys are fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shortcut {
    Unassigned,
    Live,
    Projects,
    Conversations,
    Notifications,
    Refresh,
    FollowLatest,
    PreviousProject,
    NextProject,
    PreviousConversation,
    NextConversation,
    OpenEditor,
    OpenTerminal,
    OpenProject,
    OpenBrowser,
}

impl Shortcut {
    pub const ALL: [Self; 14] = [
        Self::Live,
        Self::Projects,
        Self::Conversations,
        Self::Notifications,
        Self::Refresh,
        Self::FollowLatest,
        Self::PreviousProject,
        Self::NextProject,
        Self::PreviousConversation,
        Self::NextConversation,
        Self::OpenEditor,
        Self::OpenTerminal,
        Self::OpenProject,
        Self::OpenBrowser,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiPreferences {
    pub schema_version: u8,
    pub language: UiLanguage,
    pub shortcuts: [Shortcut; 8],
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            language: UiLanguage::Korean,
            shortcuts: [Shortcut::Unassigned; 8],
        }
    }
}

impl UiPreferences {
    pub fn assign(&mut self, key: usize, action: Option<Shortcut>) -> bool {
        let Some(slot) = key
            .checked_sub(2)
            .and_then(|index| self.shortcuts.get_mut(index))
        else {
            return false;
        };
        *slot = action.unwrap_or(Shortcut::Unassigned);
        true
    }

    pub fn action(&self, key: usize) -> Option<Shortcut> {
        key.checked_sub(2)
            .and_then(|index| self.shortcuts.get(index))
            .copied()
            .filter(|action| *action != Shortcut::Unassigned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_actions_cannot_replace_decision_keys_or_escape_the_grid() {
        let mut preferences = UiPreferences::default();
        for key in [0, 1, 10, usize::MAX] {
            assert!(!preferences.assign(key, Some(Shortcut::OpenTerminal)));
            assert_eq!(preferences.action(key), None);
        }
        assert!(preferences.assign(2, Some(Shortcut::Refresh)));
        assert_eq!(preferences.action(2), Some(Shortcut::Refresh));
        assert!(preferences.assign(2, None));
        assert_eq!(preferences.action(2), None);
    }
}
