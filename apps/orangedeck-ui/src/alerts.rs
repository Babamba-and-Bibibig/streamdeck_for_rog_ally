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

    #[cfg(test)]
    pub fn unread(&self) -> usize {
        self.entries.iter().filter(|entry| !entry.read).count()
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
        // Older Agents do not provide turn IDs. Only include notices after a known turn start.
        (None, _) => thread
            .activity
            .as_ref()
            .and_then(|activity| activity.started_at)
            .is_some_and(|at| alert.at >= at),
        (Some(_), None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unread_and_acknowledgement_belong_to_one_turn_in_one_thread() {
        let mut center = AlertCenter::default();
        let thread: CodexThreadDto = serde_json::from_value(serde_json::json!({
            "id":"thread","cwd":"/a","title":"t","preview":"","status":"working","ownership":"external_read_only","updated_at":100,"active_turn_id":"new"
        })).unwrap();
        let id = Uuid::new_v4();
        let mut old = notification(id);
        old.turn_id = Some("old".to_owned());
        center.record(old, id, Utc::now(), true, false);
        assert!(!center.current_unread(&thread));
        let id = Uuid::new_v4();
        let mut other = notification(id);
        other.thread_id = Some("other-project-thread".to_owned());
        other.turn_id = Some("new".to_owned());
        center.record(other, id, Utc::now(), true, false);
        assert!(!center.current_unread(&thread));
        let id = Uuid::new_v4();
        let mut current = notification(id);
        current.turn_id = Some("new".to_owned());
        center.record(current, id, Utc::now(), true, false);
        assert!(center.current_unread(&thread));
        center.mark_current_read(&thread);
        assert!(!center.current_unread(&thread));
        assert_eq!(center.unread(), 2);
    }

    fn notification(id: Uuid) -> NotificationDto {
        NotificationDto {
            turn_id: Some("turn".to_owned()),
            id: Some(id),
            thread_id: Some("thread".to_owned()),
            created_at: Some(Utc::now()),
            level: NotificationLevelDto::Success,
            title: "응답 완료".to_owned(),
            body: "프로젝트".to_owned(),
        }
    }

    #[test]
    fn reconnect_deduplicates_history_and_preserves_read_state() {
        let mut center = AlertCenter::default();
        let id = Uuid::new_v4();
        center.record(notification(id), id, Utc::now(), true, false);
        assert_eq!(center.unread(), 1);
        assert_eq!(center.take_announcements().count(), 1);
        center.mark_read(id);
        center.record(notification(id), id, Utc::now(), true, false);
        assert_eq!(center.entries.len(), 1);
        assert_eq!(center.unread(), 0);
        assert_eq!(center.take_announcements().count(), 0);
    }

    #[test]
    fn restored_history_does_not_popup_and_memory_is_bounded() {
        let mut center = AlertCenter::default();
        for _ in 0..600 {
            let id = Uuid::new_v4();
            center.record(notification(id), id, Utc::now(), false, true);
        }
        assert_eq!(center.entries.len(), HISTORY);
        assert_eq!(center.seen.len(), SEEN);
        assert_eq!(center.unread(), 0);
        assert_eq!(center.take_announcements().count(), 0);
    }
}
