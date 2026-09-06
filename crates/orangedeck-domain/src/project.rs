use std::{collections::BTreeMap, fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProjectIdError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(ProjectIdError(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ProjectId {
    type Err = ProjectIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid project id `{0}`; use 1-64 ASCII letters, digits, dashes, or underscores")]
pub struct ProjectIdError(String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub browser_url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectRegistry {
    projects: BTreeMap<ProjectId, Project>,
}

impl ProjectRegistry {
    pub fn new(projects: impl IntoIterator<Item = Project>) -> Result<Self, ProjectRegistryError> {
        let mut registered = BTreeMap::new();
        for project in projects {
            if !project.path.is_absolute() {
                return Err(ProjectRegistryError::PathNotAbsolute {
                    id: project.id,
                    path: project.path,
                });
            }
            if registered.insert(project.id.clone(), project).is_some() {
                return Err(ProjectRegistryError::DuplicateId);
            }
        }
        Ok(Self {
            projects: registered,
        })
    }

    pub fn resolve(&self, id: &ProjectId) -> Result<&Project, ProjectRegistryError> {
        self.projects
            .get(id)
            .ok_or_else(|| ProjectRegistryError::NotRegistered(id.clone()))
    }

    pub fn contains(&self, id: &ProjectId) -> bool {
        self.projects.contains_key(id)
    }

    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }

    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectRegistryError {
    #[error("project id is registered more than once")]
    DuplicateId,
    #[error("project `{id}` path must be absolute: {path}")]
    PathNotAbsolute { id: ProjectId, path: PathBuf },
    #[error("project `{0}` is not in the agent allow-list")]
    NotRegistered(ProjectId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, path: &str) -> Project {
        Project {
            id: ProjectId::new(id).expect("test id is valid"),
            name: id.to_owned(),
            path: PathBuf::from(path),
            browser_url: None,
        }
    }

    #[test]
    fn resolves_only_registered_project_ids() {
        let registry = ProjectRegistry::new([project("orange", "/tmp/orange")])
            .expect("registry should be valid");

        assert!(registry.resolve(&ProjectId::new("orange").unwrap()).is_ok());
        assert_eq!(
            registry.resolve(&ProjectId::new("secret").unwrap()),
            Err(ProjectRegistryError::NotRegistered(
                ProjectId::new("secret").unwrap()
            ))
        );
    }

    #[test]
    fn rejects_relative_paths_and_duplicate_ids() {
        let relative = ProjectRegistry::new([project("orange", "relative/path")]);
        assert!(matches!(
            relative,
            Err(ProjectRegistryError::PathNotAbsolute { .. })
        ));

        let duplicate =
            ProjectRegistry::new([project("orange", "/tmp/one"), project("orange", "/tmp/two")]);
        assert_eq!(duplicate, Err(ProjectRegistryError::DuplicateId));
    }
}
