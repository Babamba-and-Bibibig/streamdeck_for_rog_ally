use std::collections::{BTreeMap, VecDeque};

use chrono::Utc;

use crate::{
    CodexSnapshot, ConnectionState, GitSnapshot, HostSnapshot, JobRecord, Project, ProjectId,
    SystemSnapshot,
};

#[derive(Clone, Debug)]
pub struct DashboardState {
    pub host: HostSnapshot,
    pub projects: Vec<Project>,
    pub selected_project: Option<ProjectId>,
    pub codex: CodexSnapshot,
    pub jobs: BTreeMap<uuid::Uuid, JobRecord>,
    pub git: BTreeMap<ProjectId, GitSnapshot>,
    pub system: SystemSnapshot,
    pub activity: VecDeque<String>,
}

impl DashboardState {
    pub fn new(host_name: impl Into<String>, projects: Vec<Project>) -> Self {
        let selected_project = projects.first().map(|project| project.id.clone());
        Self {
            host: HostSnapshot {
                name: host_name.into(),
                os: std::env::consts::OS.to_owned(),
                architecture: std::env::consts::ARCH.to_owned(),
                address: None,
                connection: ConnectionState::Connected,
                tailscale: false,
                latency_ms: None,
                last_seen: Utc::now(),
            },
            projects,
            selected_project,
            codex: CodexSnapshot::default(),
            jobs: BTreeMap::new(),
            git: BTreeMap::new(),
            system: SystemSnapshot::default(),
            activity: VecDeque::new(),
        }
    }

    pub fn push_activity(&mut self, message: impl Into<String>) {
        const MAX_ACTIVITY: usize = 100;
        self.activity.push_back(message.into());
        while self.activity.len() > MAX_ACTIVITY {
            self.activity.pop_front();
        }
    }
}
