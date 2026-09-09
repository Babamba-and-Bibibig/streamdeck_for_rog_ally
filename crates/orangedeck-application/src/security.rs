use orangedeck_domain::{JobKind, ProjectId, ProjectRegistry, ProjectRegistryError};
use orangedeck_protocol::ClientCommand;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidatedCommand {
    SelectProject(ProjectId),
    RefreshState,
    RunCargo {
        project_id: ProjectId,
        kind: JobKind,
    },
    CancelJob(Uuid),
    RefreshGit(ProjectId),
    OpenEditor(ProjectId),
    OpenTerminal(ProjectId),
    OpenBrowser(ProjectId),
    OpenProject(ProjectId),
    CodexRefreshThreads,
    CodexReadThread(String),
    CodexWatchThreads(Vec<String>),
    OpenCodexChange {
        thread_id: String,
        turn_id: String,
        path: String,
    },
    RegisterCodexProject {
        thread_id: String,
        turn_id: String,
        expected_cwd: String,
        path: String,
    },
    CodexStartThread(ProjectId),
    CodexSendPrompt {
        thread_id: String,
        prompt: String,
    },
    CodexInterrupt {
        thread_id: String,
        turn_id: String,
    },
    CodexApprovalResponse {
        approval_id: Uuid,
        approve: bool,
    },
    DemoScenario(orangedeck_protocol::DemoScenario),
}

pub fn validate_command(
    command: &ClientCommand,
    projects: &ProjectRegistry,
) -> Result<ValidatedCommand, CommandValidationError> {
    use orangedeck_protocol::ApprovalDecisionDto;

    let project = |raw: &str| -> Result<ProjectId, CommandValidationError> {
        let id = ProjectId::new(raw)?;
        projects.resolve(&id)?;
        Ok(id)
    };

    match command {
        ClientCommand::SelectProject { project_id } => {
            Ok(ValidatedCommand::SelectProject(project(project_id)?))
        }
        ClientCommand::RefreshState => Ok(ValidatedCommand::RefreshState),
        ClientCommand::RunCargoCheck { project_id } => Ok(ValidatedCommand::RunCargo {
            project_id: project(project_id)?,
            kind: JobKind::CargoCheck,
        }),
        ClientCommand::RunCargoTest { project_id } => Ok(ValidatedCommand::RunCargo {
            project_id: project(project_id)?,
            kind: JobKind::CargoTest,
        }),
        ClientCommand::RunCargoClippy { project_id } => Ok(ValidatedCommand::RunCargo {
            project_id: project(project_id)?,
            kind: JobKind::CargoClippy,
        }),
        ClientCommand::RunCargoFmt { project_id } => Ok(ValidatedCommand::RunCargo {
            project_id: project(project_id)?,
            kind: JobKind::CargoFmtCheck,
        }),
        ClientCommand::RunCargoBuild { project_id } => Ok(ValidatedCommand::RunCargo {
            project_id: project(project_id)?,
            kind: JobKind::CargoBuild,
        }),
        ClientCommand::CancelJob { job_id } => Ok(ValidatedCommand::CancelJob(*job_id)),
        ClientCommand::RefreshGit { project_id } => {
            Ok(ValidatedCommand::RefreshGit(project(project_id)?))
        }
        ClientCommand::OpenEditor { project_id } => {
            Ok(ValidatedCommand::OpenEditor(project(project_id)?))
        }
        ClientCommand::OpenTerminal { project_id } => {
            Ok(ValidatedCommand::OpenTerminal(project(project_id)?))
        }
        ClientCommand::OpenBrowser { project_id } => {
            Ok(ValidatedCommand::OpenBrowser(project(project_id)?))
        }
        ClientCommand::OpenProject { project_id } => {
            Ok(ValidatedCommand::OpenProject(project(project_id)?))
        }
        ClientCommand::CodexRefreshThreads => Ok(ValidatedCommand::CodexRefreshThreads),
        ClientCommand::CodexReadThread { thread_id } => {
            validate_identifier("thread id", thread_id)?;
            Ok(ValidatedCommand::CodexReadThread(thread_id.clone()))
        }
        ClientCommand::CodexStartThread { project_id } => {
            Ok(ValidatedCommand::CodexStartThread(project(project_id)?))
        }
        ClientCommand::CodexWatchThreads { thread_ids } => {
            if thread_ids.len() > 5 {
                return Err(CommandValidationError::InvalidIdentifier(
                    "conversation slots",
                ));
            }
            for id in thread_ids {
                validate_identifier("thread id", id)?;
            }
            let mut ids = thread_ids.clone();
            ids.sort();
            ids.dedup();
            Ok(ValidatedCommand::CodexWatchThreads(ids))
        }
        ClientCommand::OpenCodexChange {
            thread_id,
            turn_id,
            path,
            ..
        } => {
            validate_identifier("thread id", thread_id)?;
            validate_identifier("turn id", turn_id)?;
            if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
                return Err(CommandValidationError::InvalidIdentifier("changed file"));
            }
            Ok(ValidatedCommand::OpenCodexChange {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                path: path.clone(),
            })
        }
        ClientCommand::RegisterCodexProject {
            thread_id,
            turn_id,
            expected_cwd,
            path,
            ..
        } => {
            validate_identifier("thread id", thread_id)?;
            validate_identifier("turn id", turn_id)?;
            for value in [expected_cwd, path] {
                if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
                    return Err(CommandValidationError::InvalidIdentifier("editor project"));
                }
            }
            if !std::path::Path::new(expected_cwd).is_absolute() {
                return Err(CommandValidationError::InvalidIdentifier("editor project"));
            }
            Ok(ValidatedCommand::RegisterCodexProject {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                expected_cwd: expected_cwd.clone(),
                path: path.clone(),
            })
        }
        ClientCommand::CodexSendPrompt { thread_id, prompt } => {
            validate_identifier("thread id", thread_id)?;
            let prompt = prompt.trim();
            if prompt.is_empty() || prompt.chars().count() > 16_000 {
                return Err(CommandValidationError::InvalidPrompt);
            }
            Ok(ValidatedCommand::CodexSendPrompt {
                thread_id: thread_id.clone(),
                prompt: prompt.to_owned(),
            })
        }
        ClientCommand::CodexInterrupt { thread_id, turn_id } => {
            validate_identifier("thread id", thread_id)?;
            validate_identifier("turn id", turn_id)?;
            Ok(ValidatedCommand::CodexInterrupt {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            })
        }
        ClientCommand::CodexApprovalResponse {
            approval_id,
            decision,
        } => Ok(ValidatedCommand::CodexApprovalResponse {
            approval_id: *approval_id,
            approve: matches!(decision, ApprovalDecisionDto::Approve),
        }),
        ClientCommand::DemoScenario { scenario } => Ok(ValidatedCommand::DemoScenario(*scenario)),
    }
}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), CommandValidationError> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err(CommandValidationError::InvalidIdentifier(label))
    } else {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CommandValidationError {
    #[error(transparent)]
    InvalidProjectId(#[from] orangedeck_domain::ProjectIdError),
    #[error(transparent)]
    Project(#[from] ProjectRegistryError),
    #[error("prompt must contain 1-16000 characters")]
    InvalidPrompt,
    #[error("invalid {0}")]
    InvalidIdentifier(&'static str),
}
