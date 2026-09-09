//! Recover successful apply_patch records when persisted app-server items omit them.
use orangedeck_domain::TurnChanges;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct RecordedEdits {
    pending: HashMap<String, String>,
    completed: Vec<Value>,
    bytes: usize,
    incomplete: bool,
}

impl RecordedEdits {
    pub(super) fn consume(&mut self, payload: &Value) {
        let Some(call_id) = payload["call_id"].as_str() else {
            return;
        };
        match payload["type"].as_str() {
            Some("custom_tool_call" | "function_call") => {
                let name = payload["name"].as_str().unwrap_or("");
                if !matches!(name, "apply_patch" | "functions.apply_patch") {
                    return;
                }
                if self.pending.len() >= 64 {
                    self.incomplete = true;
                    return;
                }
                let input = payload["input"].as_str().map(str::to_owned).or_else(|| {
                    let args: Value = serde_json::from_str(payload["arguments"].as_str()?).ok()?;
                    args["patch"]
                        .as_str()
                        .or_else(|| args["input"].as_str())
                        .map(str::to_owned)
                });
                if let Some(input) =
                    input.filter(|text| text.len() <= 65_536 && self.bytes + text.len() <= 262_144)
                {
                    self.bytes += input.len();
                    self.pending.insert(call_id.to_owned(), input);
                } else {
                    self.incomplete = true;
                }
            }
            Some("custom_tool_call_output" | "function_call_output") => {
                let Some(patch) = self.pending.remove(call_id) else {
                    return;
                };
                let output = payload["output"].as_str().unwrap_or("");
                let decoded = serde_json::from_str::<Value>(output).ok();
                let output = decoded
                    .as_ref()
                    .and_then(|value| value["output"].as_str())
                    .unwrap_or(output);
                if !output
                    .trim_start()
                    .starts_with("Success. Updated the following files:")
                    || self.completed.len() >= 64
                {
                    return;
                }
                if let Some(changes) = patch_changes(&patch) {
                    self.completed
                        .push(json!({"type":"fileChange","status":"completed","changes":changes}));
                } else {
                    self.incomplete = true;
                }
            }
            _ => {}
        }
    }

    pub(super) fn changes(&self) -> Option<TurnChanges> {
        if self.completed.is_empty() && !self.incomplete {
            return None;
        }
        let mut changes = super::changes::parse_changes(&json!({"items":self.completed}))?;
        changes.truncated |= self.incomplete;
        Some(changes)
    }
}

fn patch_changes(patch: &str) -> Option<Vec<Value>> {
    let mut lines = patch.lines();
    if lines.next()? != "*** Begin Patch" {
        return None;
    }
    let mut changes: Vec<Value> = Vec::new();
    for line in lines {
        if line == "*** End Patch" {
            return Some(changes);
        }
        let header = [
            ("*** Add File: ", "add"),
            ("*** Update File: ", "update"),
            ("*** Delete File: ", "delete"),
        ]
        .into_iter()
        .find_map(|(prefix, kind)| line.strip_prefix(prefix).map(|path| (path, kind)));
        if let Some((path, kind)) = header {
            if path.is_empty()
                || path.len() > 4096
                || path.chars().any(char::is_control)
                || changes.len() >= 64
            {
                return None;
            }
            changes.push(json!({"path":path,"kind":{"type":kind},"diff":""}));
        } else if let Some(destination) = line.strip_prefix("*** Move to: ") {
            changes.last_mut()?["kind"]["move_path"] = destination.into();
        } else if line != "*** End of File" {
            let change = changes.last_mut()?;
            let mut diff = change["diff"].as_str()?.to_owned();
            diff.push_str(line);
            diff.push('\n');
            change["diff"] = diff.into();
        }
    }
    None
}
