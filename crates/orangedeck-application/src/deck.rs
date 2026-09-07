//! Resolve a chosen shortcut without executing IO or accepting arbitrary commands.
use orangedeck_domain::Shortcut;
use orangedeck_protocol::{ClientCommand, SnapshotDto};

#[derive(Clone, Debug, PartialEq)]
pub enum ShortcutEffect {
    Local(Shortcut),
    Remote(ClientCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutUnavailable {
    Disconnected,
    UnregisteredProject,
    MissingBrowser,
}

pub fn resolve_shortcut(
    action: Shortcut,
    snapshot: Option<&SnapshotDto>,
    project_path: Option<&str>,
    connected: bool,
) -> Result<ShortcutEffect, ShortcutUnavailable> {
    use Shortcut::{OpenBrowser, OpenEditor, OpenProject, OpenTerminal, Refresh};
    if !matches!(
        action,
        Refresh | OpenEditor | OpenTerminal | OpenProject | OpenBrowser
    ) {
        return Ok(ShortcutEffect::Local(action));
    }
    if !connected {
        return Err(ShortcutUnavailable::Disconnected);
    }
    if action == Refresh {
        return Ok(ShortcutEffect::Remote(ClientCommand::CodexRefreshThreads));
    }
    let project = snapshot
        .and_then(|snapshot| {
            snapshot.projects.iter().find(|project| {
                project_path.is_some_and(|path| {
                    std::path::Path::new(path) == std::path::Path::new(&project.path)
                })
            })
        })
        .ok_or(ShortcutUnavailable::UnregisteredProject)?;
    let project_id = project.id.clone();
    let command = match action {
        OpenEditor => ClientCommand::OpenEditor { project_id },
        OpenTerminal => ClientCommand::OpenTerminal { project_id },
        OpenProject => ClientCommand::OpenProject { project_id },
        OpenBrowser if project.has_browser_url => ClientCommand::OpenBrowser { project_id },
        _ => return Err(ShortcutUnavailable::MissingBrowser),
    };
    Ok(ShortcutEffect::Remote(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_navigation_remains_local_but_host_actions_require_a_registered_project() {
        assert_eq!(
            resolve_shortcut(Shortcut::Live, None, None, false),
            Ok(ShortcutEffect::Local(Shortcut::Live))
        );
        assert_eq!(
            resolve_shortcut(Shortcut::OpenTerminal, None, Some("/mock/project"), false),
            Err(ShortcutUnavailable::Disconnected)
        );
        assert_eq!(
            resolve_shortcut(Shortcut::OpenTerminal, None, Some("/mock/project"), true),
            Err(ShortcutUnavailable::UnregisteredProject)
        );
        assert_eq!(
            resolve_shortcut(Shortcut::Refresh, None, None, true),
            Ok(ShortcutEffect::Remote(ClientCommand::CodexRefreshThreads))
        );
    }
}
