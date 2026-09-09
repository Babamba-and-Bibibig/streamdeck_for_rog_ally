use std::collections::{HashSet, VecDeque};

use chrono::{DateTime, Utc};
use orangedeck_protocol::{ApprovalDto, CodexThreadDto, NotificationDto, NotificationLevelDto};
use uuid::Uuid;

const HISTORY: usize = 100;
const SEEN: usize = 512;

#[derive(Clone, Debug)]
pub struct Alert {
    pub id: Uuid,
    pub notification: NotificationDto,
    pub at: DateTime<Utc>,
    pub read: bool,
}

#[derive(Default)]
pub struct AlertCenter {
    pub entries: VecDeque<Alert>,
    seen: HashSet<Uuid>,
    seen_order: VecDeque<Uuid>,
    announcements: VecDeque<Alert>,
}

impl AlertCenter {
    pub fn record(
        &mut self,
        notification: NotificationDto,
        fallback_id: Uuid,
        at: DateTime<Utc>,
        announce: bool,
        read: bool,
    ) {
        let id = notification.id.unwrap_or(fallback_id);
        if !self.seen.insert(id) {
            return;
        }
        self.seen_order.push_back(id);
        while self.seen_order.len() > SEEN {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        let alert = Alert {
            id,
            at: notification.created_at.unwrap_or(at),
            notification,
            read,
        };
        if announce {
            self.announcements.push_back(alert.clone());
            while self.announcements.len() > HISTORY {
                self.announcements.pop_front();
            }
        }
        self.entries.push_front(alert);
        self.entries.truncate(HISTORY);
    }

    pub fn approval(&mut self, approval: &ApprovalDto) {
        self.record(
            NotificationDto {
                turn_id: approval.turn_id.clone(),
                id: Some(approval.id),
                thread_id: approval.thread_id.clone(),
                created_at: Some(approval.requested_at),
                level: NotificationLevelDto::Warning,
                title: "승인 요청".to_owned(),
                body: approval.summary.clone(),
            },
            approval.id,
            approval.requested_at,
            true,
            false,
        );
    }

    pub fn current_unread(&self, thread: &CodexThreadDto) -> bool {
        self.entries
            .iter()
            .any(|entry| !entry.read && is_current(entry, thread))
    }

    pub fn mark_read(&mut self, id: Uuid) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.read = true;
        }
    }

    pub fn mark_current_read(&mut self, thread: &CodexThreadDto) {
        for entry in &mut self.entries {
            if is_current(entry, thread) {
                entry.read = true;
            }
        }
    }

    pub fn take_announcements(&mut self) -> impl Iterator<Item = Alert> + '_ {
        self.announcements.drain(..)
    }
}

pub fn is_current(alert: &Alert, thread: &CodexThreadDto) -> bool {
    if alert.notification.thread_id.as_deref() != Some(&thread.id) {
        return false;
    }
    match (
        alert.notification.turn_id.as_deref(),
        crate::selection::turn_id(thread),
    ) {
        (Some(a), Some(b)) => a == b,
        // Older Connectors do not provide turn IDs. Only include notices after a known turn start.
        (None, _) => thread
            .activity
            .as_ref()
            .and_then(|activity| activity.started_at)
            .is_some_and(|at| alert.at >= at),
        (Some(_), None) => false,
    }
}
