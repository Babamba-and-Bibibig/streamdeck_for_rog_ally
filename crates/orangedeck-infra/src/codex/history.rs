//! Read complete persisted items without resuming or subscribing to a conversation.
use super::{CodexClient, CodexError};
use serde_json::{Value, json};

impl CodexClient {
    pub(super) async fn read_history(&self, thread_id: &str) -> Result<Value, CodexError> {
        let summary = self
            .request(
                "thread/read",
                Some(json!({
                    "threadId": thread_id, "includeTurns": false
                })),
            )
            .await?;
        let mut thread = summary
            .get("thread")
            .cloned()
            .ok_or_else(|| CodexError::Protocol("thread/read has no thread".into()))?;
        if thread["id"].as_str() != Some(thread_id) {
            return Err(CodexError::Protocol(
                "thread/read returned a different conversation".into(),
            ));
        }
        match self.read_latest_turn(thread_id).await {
            Ok(turn) => thread["turns"] = turn.into_iter().collect::<Vec<_>>().into(),
            Err(error)
                if pagination_unsupported(&error) && thread["historyMode"] != "paginated" =>
            {
                let response = self
                    .request(
                        "thread/read",
                        Some(json!({
                            "threadId": thread_id, "includeTurns": true
                        })),
                    )
                    .await?;
                thread = response
                    .get("thread")
                    .cloned()
                    .filter(|thread| thread["id"].as_str() == Some(thread_id))
                    .ok_or_else(|| {
                        CodexError::Protocol("thread/read returned invalid history".into())
                    })?;
            }
            Err(error) => return Err(error),
        }
        Ok(thread)
    }

    async fn read_latest_turn(&self, thread_id: &str) -> Result<Option<Value>, CodexError> {
        let mut cursor = Value::Null;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..8 {
            let response = self
                .request(
                    "thread/turns/list",
                    Some(json!({
                        "threadId":thread_id, "limit":20, "sortDirection":"desc",
                        "itemsView":"full", "cursor":cursor
                    })),
                )
                .await?;
            let turns = response["data"]
                .as_array()
                .ok_or_else(|| CodexError::Protocol("turn page has no data".into()))?;
            for original in turns {
                let mut turn = original.clone();
                if turn["id"].as_str().is_none_or(str::is_empty) {
                    return Err(CodexError::Protocol("turn has no id".into()));
                }
                // Summary/omitted items cannot establish that a turn had no edits.
                // A full turn already contains its items; do not fetch them twice.
                if !turn["items"].is_array()
                    || turn["itemsView"]
                        .as_str()
                        .is_some_and(|view| view != "full")
                {
                    turn["items"] = self.read_turn_items(thread_id, &turn).await?.into();
                    turn["itemsView"] = "full".into();
                }
                if has_user_message(&turn) {
                    return Ok(Some(turn));
                }
            }
            cursor = response.get("nextCursor").cloned().unwrap_or(Value::Null);
            if cursor.is_null() {
                return Ok(None);
            }
            if !cursor.is_string() || !seen.insert(cursor.to_string()) {
                return Err(CodexError::Protocol(
                    "invalid turn pagination cursor".into(),
                ));
            }
        }
        Err(CodexError::Protocol(
            "turn history exceeds read limit".into(),
        ))
    }

    async fn read_turn_items(
        &self,
        thread_id: &str,
        turn: &Value,
    ) -> Result<Vec<Value>, CodexError> {
        let turn_id = turn["id"]
            .as_str()
            .ok_or_else(|| CodexError::Protocol("turn has no id".into()))?;
        let mut items = Vec::new();
        let mut bytes = 0;
        let mut cursor = Value::Null;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..32 {
            let response = self
                .request(
                    "thread/items/list",
                    Some(json!({
                        "threadId":thread_id, "turnId":turn_id, "limit":100,
                        "sortDirection":"asc", "cursor":cursor
                    })),
                )
                .await?;
            let entries = response["data"]
                .as_array()
                .ok_or_else(|| CodexError::Protocol("item page has no data".into()))?;
            for entry in entries {
                if entry["turnId"].as_str() != Some(turn_id) || !entry["item"].is_object() {
                    return Err(CodexError::Protocol(
                        "item page contains another turn".into(),
                    ));
                }
                bytes += entry["item"].to_string().len();
                if items.len() >= 3200 || bytes > 8 * 1024 * 1024 {
                    return Err(CodexError::Protocol("turn items exceed read limit".into()));
                }
                items.push(entry["item"].clone());
            }
            cursor = response.get("nextCursor").cloned().unwrap_or(Value::Null);
            if cursor.is_null() {
                return Ok(items);
            }
            if !cursor.is_string() || !seen.insert(cursor.to_string()) {
                return Err(CodexError::Protocol(
                    "invalid item pagination cursor".into(),
                ));
            }
        }
        Err(CodexError::Protocol(
            "item history exceeds read limit".into(),
        ))
    }
}

fn has_user_message(turn: &Value) -> bool {
    turn["items"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["type"] == "userMessage"))
}

fn pagination_unsupported(error: &CodexError) -> bool {
    matches!(error, CodexError::Rpc(failure) if matches!(failure.code, -32601 | -32602))
}
