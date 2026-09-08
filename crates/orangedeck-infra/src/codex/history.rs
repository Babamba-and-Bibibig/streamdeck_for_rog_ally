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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn complete_file_items_are_read_across_pages_without_resuming_the_thread() {
        let temp = tempfile::tempdir().unwrap();
        for mode in ["full", "omitted", "paged", "legacy", "wrong", "cycle"] {
            let executable = temp.path().join(format!("fake-codex-{mode}"));
            std::fs::write(&executable, r"#!/usr/bin/env python3
import json,sys
from pathlib import Path
mode=Path(__file__).name.rsplit('-',1)[-1]
if '--version' in sys.argv:
    print('codex-cli 0.153.4');sys.exit(0)
if '--help' in sys.argv:
    print('generate-json-schema');sys.exit(0)
user={'type':'userMessage','content':[{'type':'text','text':'Make the test edit'}]}
edit={'type':'fileChange','status':'completed','changes':[{'path':'src/new.rs','kind':{'type':'add'},'diff':'@@ -0,0 +42 @@\n+created'}]}
for line in sys.stdin:
    req=json.loads(line)
    if 'id' not in req:continue
    method=req['method'];params=req['params']
    if method=='initialize':
        assert params['capabilities']['experimentalApi'];result={}
    elif method=='thread/read':
        items=[user,edit] if mode=='legacy' else [user]
        result={'thread':{'id':'listed','cwd':'/mock/project','status':{'type':'notLoaded'},'historyMode':'legacy' if mode=='legacy' else 'paginated','turns':[{'id':'turn','status':'completed','items':items}] if params['includeTurns'] else []}}
    elif method=='thread/turns/list':
        if mode=='legacy':
            print(json.dumps({'id':req['id'],'error':{'code':-32601,'message':'unsupported'}}),flush=True);continue
        assert params['itemsView']=='full'
        turn={'id':'turn','status':'completed','itemsView':'full' if mode in ('full','omitted') else 'summary'}
        if mode!='omitted':turn['items']=[user,edit] if mode=='full' else [user]
        result={'data':[turn],'nextCursor':None}
    elif method=='thread/items/list':
        assert mode!='full','Complete turn items must not be fetched a second time'
        assert params['threadId']=='listed' and params['turnId']=='turn'
        if params['cursor'] is None:
            result={'data':[{'turnId':'turn','item':user}],'nextCursor':'next'}
        else:
            assert params['cursor']=='next'
            result={'data':[{'turnId':'other' if mode=='wrong' else 'turn','item':edit}],'nextCursor':'next' if mode=='cycle' else None}
    else:
        raise AssertionError('No resume, subscription or control allowed: '+method)
    print(json.dumps({'id':req['id'],'result':result}),flush=True)
").unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let client = CodexClient::spawn(&executable, temp.path().join("owned.json"), vec![])
                .await
                .unwrap();
            let result = client.read_thread("listed").await;
            if matches!(mode, "full" | "omitted" | "paged" | "legacy") {
                let changes = result.unwrap().observation.unwrap().changes.unwrap();
                assert_eq!(changes.files.len(), 1);
                assert_eq!(changes.files[0].path, "src/new.rs");
                assert_eq!(changes.files[0].first_line, 42);
            } else {
                assert!(result.is_err());
            }
            client.shutdown.cancel();
        }
    }
}
