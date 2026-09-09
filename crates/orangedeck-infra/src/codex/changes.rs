//! Read the completed fileChange items of one turn. Never infer authorship from Git status.
use orangedeck_domain::{CodeChange, CodeChangeKind, TurnChanges, is_file_change_path};
use serde_json::Value;

pub(super) fn parse_changes(turn: &Value) -> Option<TurnChanges> {
    if turn
        .get("itemsView")
        .and_then(Value::as_str)
        .is_some_and(|view| view != "full")
    {
        return None;
    }
    let items = turn.get("items")?.as_array()?;
    let mut result = TurnChanges::default();
    for item in items {
        // A tool can change files without producing a fileChange item. Missing
        // records are not evidence of no edits; never disable the UI as "none".
        if matches!(
            item.get("type").and_then(Value::as_str),
            Some("commandExecution" | "mcpToolCall" | "dynamicToolCall" | "functionCallOutput")
        ) {
            result.truncated = true;
        }
        if item.get("type").and_then(Value::as_str) != Some("fileChange") {
            continue;
        }
        match item.get("status").and_then(Value::as_str) {
            Some("completed") => {}
            Some("failed" | "declined" | "inProgress") => continue,
            _ => {
                result.truncated = true;
                continue;
            }
        }
        let Some(changes) = item.get("changes").and_then(Value::as_array) else {
            result.truncated = true;
            continue;
        };
        for change in changes {
            let Some(path) = change
                .get("path")
                .and_then(Value::as_str)
                .filter(|path| valid_path(path))
            else {
                result.truncated = true;
                continue;
            };
            let kind = &change["kind"];
            let moved = kind.get("move_path").and_then(Value::as_str);
            if moved.is_some_and(|path| !valid_path(path)) {
                result.truncated = true;
                continue;
            }
            if !is_file_change_path(path) || moved.is_some_and(|path| !is_file_change_path(path)) {
                continue;
            }
            let change_kind = match kind
                .get("type")
                .and_then(Value::as_str)
                .or_else(|| kind.as_str())
            {
                Some("add") => CodeChangeKind::Added,
                Some("delete") => CodeChangeKind::Deleted,
                Some("update") if moved.is_some() => CodeChangeKind::Renamed,
                Some("update") => CodeChangeKind::Modified,
                _ => {
                    result.truncated = true;
                    continue;
                }
            };
            let destination = moved.unwrap_or(path);
            let existing = result
                .files
                .iter()
                .position(|file| file.path == path || file.path == destination);
            if existing.is_none() && result.files.len() >= 64 {
                result.truncated = true;
                continue;
            }
            let Some(diff) = change.get("diff").and_then(Value::as_str) else {
                result.truncated = true;
                continue;
            };
            let file = CodeChange {
                path: destination.to_owned(),
                previous_path: moved.map(|_| path.to_owned()),
                kind: change_kind,
                first_line: first_changed_line(diff).unwrap_or(1),
                truncated: false,
            };
            if let Some(index) = existing {
                // Keep the latest target and line, never a copy of the source patch.
                let previous = &mut result.files[index];
                let original = previous.previous_path.clone();
                let was_added = previous.kind == CodeChangeKind::Added;
                *previous = file;
                previous.previous_path = original.or(previous.previous_path.clone());
                if was_added && previous.kind == CodeChangeKind::Modified {
                    previous.kind = CodeChangeKind::Added;
                }
            } else {
                result.files.push(file);
            }
        }
    }
    Some(result)
}

fn valid_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 4096 && !path.chars().any(char::is_control)
}

pub(super) fn first_changed_line(diff: &str) -> Option<u32> {
    for line in diff.lines() {
        if let Some(hunk) = line.strip_prefix("@@ ")
            && let Some(added) = hunk
                .split_whitespace()
                .find_map(|part| part.strip_prefix('+'))
            && let Some(line) = added
                .split(',')
                .next()
                .and_then(|value| value.parse::<u32>().ok())
        {
            return Some(line.clamp(1, 10_000_000));
        }
    }
    None
}
