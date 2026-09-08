//! Read the completed fileChange items of one turn. Never infer authorship from Git status.
use orangedeck_domain::{CodeChange, CodeChangeKind, TurnChanges};
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
    let mut remaining = 65_536;
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
            let previous_length = existing.map_or(0, |index| result.files[index].diff.len());
            let separator = usize::from(previous_length > 0 && !diff.is_empty()) * 2;
            let budget = remaining.min(16_384_usize.saturating_sub(previous_length));
            let mut length = diff.len().min(budget.saturating_sub(separator));
            while !diff.is_char_boundary(length) {
                length -= 1;
            }
            let separator = if length > 0 { separator } else { 0 };
            remaining -= length + separator;
            let truncated = length < diff.len();
            let file = CodeChange {
                path: destination.to_owned(),
                previous_path: moved.map(|_| path.to_owned()),
                kind: change_kind,
                first_line: first_changed_line(diff),
                diff: diff[..length].to_owned(),
                truncated,
            };
            if let Some(index) = existing {
                // Preserve each edit's diff, but use the latest target and location.
                let previous = &mut result.files[index];
                let mut combined = std::mem::take(&mut previous.diff);
                if separator > 0 {
                    combined.push_str("\n\n");
                }
                combined.push_str(&file.diff);
                let original = previous.previous_path.clone();
                let was_added = previous.kind == CodeChangeKind::Added;
                let clipped = previous.truncated || file.truncated;
                *previous = file;
                previous.diff = combined;
                previous.truncated = clipped;
                previous.previous_path = original.or(previous.previous_path.clone());
                if was_added && previous.kind == CodeChangeKind::Modified {
                    previous.kind = CodeChangeKind::Added;
                }
            } else {
                result.files.push(file);
            }
            result.truncated |= truncated;
        }
    }
    Some(result)
}

fn valid_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 4096 && !path.chars().any(char::is_control)
}

fn first_changed_line(diff: &str) -> u32 {
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
            return line.clamp(1, 10_000_000);
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn records_only_completed_edits_and_preserves_rename_destination_and_line() {
        let turn = json!({"items": [
            {"type":"fileChange","status":"declined","changes":[{"path":"declined.rs","kind":{"type":"add"},"diff":"+never"}]},
            {"type":"fileChange","status":"inProgress","changes":[{"path":"waiting.rs","kind":{"type":"add"},"diff":"+wait"}]},
            {"type":"fileChange","status":"completed","changes":[
                {"path":"src/old.rs","kind":{"type":"update","move_path":"src/new.rs"},"diff":"@@ -8,2 +12,3 @@\n-old\n+new"},
                {"path":"deleted.rs","kind":{"type":"delete"},"diff":"-gone"}
            ]}
        ]});
        let changes = parse_changes(&turn).unwrap();
        assert_eq!(changes.files.len(), 2);
        assert_eq!(changes.files[0].path, "src/new.rs");
        assert_eq!(changes.files[0].first_line, 12);
        assert_eq!(
            changes.files[0].previous_path.as_deref(),
            Some("src/old.rs")
        );
        assert_eq!(changes.files[1].kind, CodeChangeKind::Deleted);
    }

    #[test]
    fn no_edits_is_distinct_from_missing_partial_or_malformed_history() {
        assert_eq!(
            parse_changes(&json!({"items":[]})),
            Some(TurnChanges::default())
        );
        assert_eq!(parse_changes(&json!({})), None);
        assert_eq!(
            parse_changes(&json!({"items":[],"itemsView":"summary"})),
            None
        );
        assert!(
            parse_changes(&json!({"items":[{"type":"fileChange","status":"completed"}]}))
                .unwrap()
                .truncated
        );
    }

    #[test]
    fn bounds_diff_bytes_without_breaking_unicode_or_losing_file_targets() {
        let changes = parse_changes(
            &json!({"items":[{"type":"fileChange","status":"completed","changes":[
                {"path":"한글.rs","kind":{"type":"update"},"diff":"한글".repeat(20_000)}
            ]}]}),
        )
        .unwrap();
        assert!(changes.truncated);
        assert!(changes.files[0].truncated);
        assert!(changes.files[0].diff.len() <= 16_384);
    }

    #[test]
    fn repeated_edits_share_the_file_and_total_budget_including_separators() {
        let edits: Vec<_> = (0..500).map(|index| json!({
            "path":format!("file-{}.rs", index % 8),"kind":{"type":"update"},"diff":"한글".repeat(100)
        })).collect();
        let changes = parse_changes(
            &json!({"items":[{"type":"fileChange","status":"completed","changes":edits}]}),
        )
        .unwrap();
        assert_eq!(changes.files.len(), 8);
        assert!(changes.truncated);
        assert!(changes.files.iter().all(|file| file.diff.len() <= 16_384));
        assert!(
            changes
                .files
                .iter()
                .map(|file| file.diff.len())
                .sum::<usize>()
                <= 65_536
        );
    }
    #[test]
    fn tool_execution_without_file_records_is_unknown_not_no_edits() {
        for kind in ["commandExecution", "mcpToolCall", "dynamicToolCall"] {
            let changes =
                parse_changes(&json!({"items":[{"type":kind,"status":"completed"}]})).unwrap();
            assert!(changes.files.is_empty());
            assert!(changes.truncated);
        }
    }
}
