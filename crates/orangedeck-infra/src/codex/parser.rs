use std::collections::HashSet;

use chrono::Utc;
use orangedeck_domain::{
    AccountUsage, ApprovalKind, ApprovalRequest, CodexLimits, CodexThread, CodexThreadStatus,
    DailyTokenUsage, LimitBucket, Project, RateLimitWindow, ThreadObservation, ThreadOwnership,
    TokenUsage,
};
use serde_json::Value;
use uuid::Uuid;

use super::CodexError;

pub fn parse_thread<S: std::hash::BuildHasher>(
    value: &Value,
    owned: &HashSet<String, S>,
    projects: &[Project],
) -> Result<CodexThread, CodexError> {
    let id = string_field(value, "id")?;
    let cwd = string_field(value, "cwd")?;
    let preview = value
        .get("preview")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let title = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map_or_else(|| preview_title(&preview), ToOwned::to_owned);
    let status = parse_thread_status(value.get("status"));
    let project_id = projects
        .iter()
        .find(|project| project.path.to_string_lossy() == cwd)
        .map(|project| project.id.clone());
    Ok(CodexThread {
        ownership: if owned.contains(&id) {
            ThreadOwnership::OrangeDeck
        } else {
            ThreadOwnership::ExternalReadOnly
        },
        id,
        project_id,
        cwd,
        title,
        preview,
        status,
        updated_at: value
            .get("updatedAt")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        active_turn_id: None,
        token_usage: None,
        live_usage: None,
        activity: None,
        observation: None,
    })
}

pub fn parse_observation(value: &Value) -> Result<ThreadObservation, CodexError> {
    let turns = value
        .get("turns")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexError::Protocol("thread/read has no turns".to_owned()))?;
    let latest = turns.iter().rev().find(|turn| {
        turn.get("items")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.get("type").and_then(Value::as_str) == Some("userMessage"))
            })
    });
    let items = latest
        .and_then(|turn| turn.get("items"))
        .and_then(Value::as_array);
    let prompt = items
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("userMessage"))
        })
        .and_then(|item| item.get("content"))
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                    Some("text") => part.get("text").and_then(Value::as_str),
                    Some("image" | "localImage") => Some("[이미지 첨부]"),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        });
    let reply = items
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("agentMessage"))
        })
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str);
    Ok(ThreadObservation {
        turn_id: latest
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        latest_user_prompt: prompt
            .filter(|text| !text.trim().is_empty())
            .map(|text| truncate(&text, 4000)),
        latest_codex_reply: reply.map(|text| truncate(text, 32_000)),
        changes: latest.and_then(super::changes::parse_changes),
        last_turn_status: parse_turn_status(latest.and_then(|turn| turn.get("status"))),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        observed_at: Utc::now(),
    })
}

pub fn parse_account_usage(value: &Value) -> Result<AccountUsage, CodexError> {
    let summary = value
        .get("summary")
        .filter(|summary| summary.is_object())
        .ok_or_else(|| CodexError::Protocol("account/usage/read has no summary".to_owned()))?;
    let mut daily = value
        .get("dailyUsageBuckets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|day| {
            let date = day.get("startDate")?.as_str()?;
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
            Some(DailyTokenUsage {
                date: date.to_owned(),
                tokens: day.get("tokens")?.as_i64().filter(|n| *n >= 0)?,
            })
        })
        .collect::<Vec<_>>();
    daily.sort_by(|a, b| b.date.cmp(&a.date));
    daily.dedup_by(|a, b| a.date == b.date);
    daily.truncate(31);
    Ok(AccountUsage {
        lifetime_tokens: summary
            .get("lifetimeTokens")
            .and_then(Value::as_i64)
            .filter(|n| *n >= 0),
        peak_daily_tokens: summary
            .get("peakDailyTokens")
            .and_then(Value::as_i64)
            .filter(|n| *n >= 0),
        daily,
        updated_at: Some(Utc::now()),
        unavailable_reason: None,
    })
}

pub fn parse_thread_status(value: Option<&Value>) -> CodexThreadStatus {
    let Some(value) = value else {
        return CodexThreadStatus::Unknown;
    };
    let status_type = value
        .as_str()
        .or_else(|| value.get("type").and_then(Value::as_str));
    match status_type {
        Some("notLoaded") => CodexThreadStatus::NotLoaded,
        Some("idle") => CodexThreadStatus::Idle,
        Some("systemError") => CodexThreadStatus::Error,
        Some("active") => {
            let waiting = value
                .get("activeFlags")
                .and_then(Value::as_array)
                .is_some_and(|flags| {
                    flags.iter().any(|flag| {
                        matches!(
                            flag.as_str(),
                            Some("waitingOnApproval" | "waitingOnUserInput")
                        )
                    })
                });
            if waiting {
                CodexThreadStatus::WaitingApproval
            } else {
                CodexThreadStatus::Working
            }
        }
        _ => CodexThreadStatus::Unknown,
    }
}

pub fn parse_turn_status(value: Option<&Value>) -> CodexThreadStatus {
    match value.and_then(Value::as_str) {
        Some("inProgress") => CodexThreadStatus::Working,
        Some("completed") => CodexThreadStatus::Completed,
        Some("interrupted") => CodexThreadStatus::Idle,
        Some("failed") => CodexThreadStatus::Error,
        _ => CodexThreadStatus::Unknown,
    }
}

pub fn parse_token_usage(value: &Value) -> Result<TokenUsage, CodexError> {
    let usage = value.get("tokenUsage").unwrap_or(value);
    let total = usage.get("total").ok_or_else(|| {
        CodexError::Protocol("thread token usage notification has no total field".to_owned())
    })?;
    Ok(TokenUsage {
        input_tokens: i64_field(total, "inputTokens"),
        cached_input_tokens: i64_field(total, "cachedInputTokens"),
        output_tokens: i64_field(total, "outputTokens"),
        reasoning_output_tokens: i64_field(total, "reasoningOutputTokens"),
        total_tokens: i64_field(total, "totalTokens"),
        model_context_window: usage.get("modelContextWindow").and_then(Value::as_i64),
    })
}

pub fn parse_limits(value: &Value) -> Result<CodexLimits, CodexError> {
    let snapshot = value
        .get("rateLimitsByLimitId")
        .and_then(|all| all.get("codex"))
        .filter(|value| value.is_object())
        .or_else(|| value.get("rateLimits").filter(|value| value.is_object()))
        .unwrap_or(value);
    if !snapshot.is_object() {
        return Err(CodexError::Protocol(
            "rate-limit response has no snapshot".to_owned(),
        ));
    }
    let credits_balance = snapshot
        .get("credits")
        .and_then(|credits| credits.get("balance"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Ok(CodexLimits {
        limit_id: snapshot
            .get("limitId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        limit_name: snapshot
            .get("limitName")
            .and_then(Value::as_str)
            .map(str::to_owned),
        plan_type: snapshot
            .get("planType")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        primary: snapshot.get("primary").and_then(parse_limit_window),
        secondary: snapshot.get("secondary").and_then(parse_limit_window),
        credits_balance,
        updated_at: Some(Utc::now()),
        additional: value
            .get("rateLimitsByLimitId")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(id, bucket)| {
                id.as_str()
                    != snapshot
                        .get("limitId")
                        .and_then(Value::as_str)
                        .unwrap_or("codex")
                    && *bucket != snapshot
            })
            .map(|(id, bucket)| LimitBucket {
                id: id.clone(),
                name: bucket
                    .get("limitName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                primary: bucket.get("primary").and_then(parse_limit_window),
                secondary: bucket.get("secondary").and_then(parse_limit_window),
            })
            .collect(),
    })
}

fn parse_limit_window(value: &Value) -> Option<RateLimitWindow> {
    Some(RateLimitWindow {
        used_percent: i32::try_from(value.get("usedPercent")?.as_i64()?).ok()?,
        resets_at: value.get("resetsAt").and_then(Value::as_i64),
        window_duration_minutes: value.get("windowDurationMins").and_then(Value::as_i64),
    })
}

pub fn parse_approval(method: &str, params: &Value) -> ApprovalRequest {
    let thread_id = params
        .get("threadId")
        .or_else(|| params.get("conversationId"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let reason = params
        .get("reason")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let command_text = params
        .get("command")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned)
        })
        .filter(|value| !value.is_empty());
    let command = command_text.as_deref();
    let (kind, title) = match method {
        "item/commandExecution/requestApproval" | "execCommandApproval" => {
            (ApprovalKind::CommandExecution, "COMMAND APPROVAL")
        }
        "item/fileChange/requestApproval" | "applyPatchApproval" => {
            (ApprovalKind::FileChange, "FILE CHANGE APPROVAL")
        }
        "item/permissions/requestApproval" => (ApprovalKind::Permissions, "PERMISSION APPROVAL"),
        _ => (ApprovalKind::Other, "CODEX APPROVAL"),
    };
    let summary = command
        .or(reason)
        .unwrap_or("Codex needs an explicit decision")
        .to_owned();
    let mut details = Vec::new();
    // The deck preview is short, but the decision view must retain the complete
    // action. Never approve a full command/permission set using truncated details.
    if let Some(command) = command {
        details.push(format!("command: {command}"));
    }
    if let Some(cwd) = params.get("cwd").and_then(Value::as_str) {
        details.push(format!("cwd: {cwd}"));
    }
    if let Some(reason) = reason
        && command != Some(reason)
    {
        details.push(format!("reason: {reason}"));
    }
    if let Some(changes) = params.get("fileChanges") {
        details.push(format!("file changes: {changes}"));
    }
    if let Some(root) = params.get("grantRoot").and_then(Value::as_str) {
        details.push(format!("requested root: {root}"));
    }
    if kind == ApprovalKind::Permissions
        && let Some(permissions) = params.get("permissions")
    {
        details.push(format!("permissions: {permissions}"));
    }
    ApprovalRequest {
        id: Uuid::new_v4(),
        thread_id,
        turn_id: params
            .get("turnId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        kind,
        title: title.to_owned(),
        summary: truncate(&summary, 500),
        details,
        requested_at: Utc::now(),
    }
}

pub fn approval_result(method: &str, approve: bool, params: &Value) -> Value {
    match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            serde_json::json!({ "decision": if approve { "accept" } else { "decline" } })
        }
        "item/permissions/requestApproval" => {
            let permissions = if approve {
                params
                    .get("permissions")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            serde_json::json!({ "permissions": permissions, "scope": "turn" })
        }
        "execCommandApproval" | "applyPatchApproval" => {
            if approve {
                serde_json::json!({ "decision": "approved" })
            } else {
                serde_json::json!({
                    "decision": { "denied": { "rejection": "Rejected from OrangeDeck" } }
                })
            }
        }
        _ => serde_json::json!({ "decision": if approve { "accept" } else { "decline" } }),
    }
}

pub fn activity_from_item(params: &Value) -> (Option<String>, Option<String>, String, String) {
    let thread_id = params
        .get("threadId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let turn_id = params
        .get("turnId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let item = params.get("item").unwrap_or(params);
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("activity")
        .to_owned();
    let text = item
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| item.get("name").and_then(Value::as_str))
        .or_else(|| item.get("status").and_then(Value::as_str))
        .unwrap_or(&kind)
        .to_owned();
    (thread_id, turn_id, kind, truncate(&text, 500))
}

fn string_field(value: &Value, key: &'static str) -> Result<String, CodexError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| CodexError::Protocol(format!("Codex payload has no `{key}` string")))
}

fn i64_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn preview_title(preview: &str) -> String {
    let preview = preview.trim();
    if preview.is_empty() {
        "Untitled thread".to_owned()
    } else {
        truncate(preview, 72)
    }
}

fn truncate(value: &str, maximum: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(maximum).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::PathBuf};

    use orangedeck_domain::{ProjectId, ThreadOwnership};

    use super::*;

    #[test]
    fn long_approval_commands_keep_their_full_tail_in_details() {
        let command = format!("{} final-argument", "한글 argument ".repeat(100));
        let approval = parse_approval(
            "item/commandExecution/requestApproval",
            &serde_json::json!({
                "command": command,
                "reason": "Review the entire command",
                "transcript_path": "private-transcript-must-not-be-forwarded"
            }),
        );
        assert_eq!(approval.summary.chars().count(), 503);
        assert!(!approval.summary.contains("final-argument"));
        assert!(approval.details.contains(&format!("command: {command}")));
        assert!(
            approval
                .details
                .contains(&"reason: Review the entire command".to_owned())
        );
        assert!(!approval.details.join("\n").contains("private-transcript"));
    }

    #[test]
    fn approval_details_keep_every_file_change_and_permission() {
        let changes: serde_json::Map<String, Value> = (0..24)
            .map(|index| {
                (
                    format!("src/file-{index:02}.rs"),
                    serde_json::json!({"diff": format!("{} final-change-{index}", "+test line\n".repeat(50))}),
                )
            })
            .collect();
        let permissions = serde_json::json!({
            "fileSystem": {"write": (0..40).map(|index| format!("/mock/permission-{index:02}")).collect::<Vec<_>>()}
        });
        let params = serde_json::json!({"fileChanges": changes, "permissions": permissions});
        let approval = parse_approval("item/permissions/requestApproval", &params);
        for (prefix, field) in [
            ("file changes: ", "fileChanges"),
            ("permissions: ", "permissions"),
        ] {
            let detail = approval
                .details
                .iter()
                .find_map(|detail| detail.strip_prefix(prefix))
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(detail).unwrap(),
                params[field]
            );
        }
    }

    #[test]
    fn argument_array_approval_commands_preserve_argument_boundaries() {
        let command = serde_json::json!(["test-tool", "argument with spaces", "literal;argument"]);
        let approval = parse_approval(
            "execCommandApproval",
            &serde_json::json!({"command": command}),
        );
        let detail = approval
            .details
            .iter()
            .find_map(|detail| detail.strip_prefix("command: "))
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(detail).unwrap(), command);
    }

    #[test]
    fn complete_bucket_view_takes_precedence_over_legacy_window_and_keeps_model_scope() {
        let limits = parse_limits(&serde_json::json!({
            "rateLimits":{"limitId":"codex","primary":{"usedPercent":39,"windowDurationMins":10080}},
            "rateLimitsByLimitId":{
                "codex":{"limitId":"codex","primary":{"usedPercent":2,"windowDurationMins":300},"secondary":{"usedPercent":39,"windowDurationMins":10080}},
                "spark":{"limitId":"spark","limitName":"Spark","primary":{"usedPercent":3,"windowDurationMins":300}}
            }
        })).unwrap();
        assert_eq!(limits.primary.unwrap().window_duration_minutes, Some(300));
        assert_eq!(limits.limit_id.as_deref(), Some("codex"));
        assert_eq!(limits.additional[0].name.as_deref(), Some("Spark"));
    }

    #[test]
    fn null_legacy_limits_keep_all_official_buckets_and_reported_durations() {
        let limits = parse_limits(&serde_json::json!({"rateLimits":null,"rateLimitsByLimitId":{
            "codex":{"primary":{"usedPercent":12,"windowDurationMins":300},"secondary":{"usedPercent":34,"windowDurationMins":10080}},
            "review":{"limitName":"Reviews","primary":{"usedPercent":56,"windowDurationMins":60}}
        }})).unwrap();
        assert_eq!(limits.primary.unwrap().window_duration_minutes, Some(300));
        assert_eq!(limits.secondary.unwrap().remaining_percent(), 66);
        assert_eq!(limits.additional.len(), 1);
        assert_eq!(
            limits.additional[0].primary.as_ref().unwrap().used_percent,
            56
        );
        let usage = parse_account_usage(&serde_json::json!({"summary":{"lifetimeTokens":1234,"peakDailyTokens":567},"dailyUsageBuckets":[]})).unwrap();
        assert_eq!(usage.peak_daily_tokens, Some(567));
    }

    #[test]
    fn observation_reads_latest_question_and_only_its_reply() {
        let value = serde_json::json!({
            "preview": "first question", "model": "test-model",
            "turns": [
                {"status":"completed", "items":[{"type":"userMessage","content":[{"type":"text","text":"first question"}]}, {"type":"agentMessage","text":"old answer"}]},
                {"status":"inProgress", "items":[{"type":"userMessage","content":[{"type":"text","text":"최근 질문"}]}, {"type":"reasoning","text":"must not be forwarded"}]}
            ]
        });
        let observation = parse_observation(&value).unwrap();
        assert_eq!(observation.latest_user_prompt.as_deref(), Some("최근 질문"));
        assert_eq!(observation.latest_codex_reply, None);
        assert_eq!(observation.last_turn_status, CodexThreadStatus::Working);
        assert_eq!(observation.model.as_deref(), Some("test-model"));
    }

    #[test]
    fn a_question_without_edits_does_not_inherit_the_previous_turns_files() {
        let observation = parse_observation(&serde_json::json!({"turns":[
            {"id":"previous","status":"completed","items":[
                {"type":"userMessage","content":[{"type":"text","text":"Edit the file"}]},
                {"type":"fileChange","status":"completed","changes":[{"path":"previous.rs","kind":{"type":"update"},"diff":"+previous"}]}
            ]},
            {"id":"current","status":"completed","items":[
                {"type":"userMessage","content":[{"type":"text","text":"Explain it, without editing"}]},
                {"type":"agentMessage","text":"This is an explanation only."}
            ]}
        ]})).unwrap();
        assert_eq!(observation.turn_id.as_deref(), Some("current"));
        assert_eq!(
            observation.changes,
            Some(orangedeck_domain::TurnChanges::default())
        );
    }

    #[test]
    fn observation_bounds_unicode_and_does_not_infer_an_empty_turn_is_complete() {
        let value = serde_json::json!({"turns":[{"status":"completed","items":[{"type":"userMessage","content":[{"type":"text","text":"한".repeat(5000)}]}]}]});
        let observation = parse_observation(&value).unwrap();
        assert_eq!(
            observation.latest_user_prompt.unwrap().chars().count(),
            4003
        );
        let empty = parse_observation(&serde_json::json!({"turns":[]})).unwrap();
        assert!(empty.latest_user_prompt.is_none());
        assert_eq!(empty.last_turn_status, CodexThreadStatus::Unknown);
        assert!(parse_observation(&serde_json::json!({"preview":"not a latest prompt"})).is_err());
    }

    #[test]
    fn account_usage_is_nullable_and_separate_from_quota_percent() {
        let absent = parse_account_usage(
            &serde_json::json!({"summary":{"lifetimeTokens":null},"dailyUsageBuckets":null}),
        )
        .unwrap();
        assert_eq!(absent.lifetime_tokens, None);
        assert!(absent.daily.is_empty());
        assert!(
            parse_account_usage(&serde_json::json!({"rateLimits":{"primary":{"usedPercent":40}}}))
                .is_err()
        );
        let usage = parse_account_usage(
            &serde_json::json!({"summary":{"lifetimeTokens":0},"dailyUsageBuckets":[
                {"startDate":"2026-09-04","tokens":100}, {"startDate":"invalid","tokens":999},
                {"startDate":"2026-09-05","tokens":200}, {"startDate":"2026-09-03","tokens":-1}
            ]}),
        )
        .unwrap();
        assert_eq!(usage.lifetime_tokens, Some(0));
        assert_eq!(usage.daily.len(), 2);
        assert_eq!(usage.daily[0].date, "2026-09-05");
        assert_eq!(usage.daily[0].tokens, 200);
    }

    #[test]
    fn parses_generated_v2_thread_shape_and_ownership() {
        let owned = HashSet::from(["thr_owned".to_owned()]);
        let projects = [Project {
            id: ProjectId::new("orange").unwrap(),
            name: "Orange".to_owned(),
            path: PathBuf::from("/tmp/orange"),
            browser_url: None,
        }];
        let payload = serde_json::json!({
            "id": "thr_owned",
            "cwd": "/tmp/orange",
            "preview": "Build the dashboard",
            "updatedAt": 123,
            "status": { "type": "active", "activeFlags": ["waitingOnApproval"] }
        });
        let thread = parse_thread(&payload, &owned, &projects).unwrap();

        assert_eq!(thread.ownership, ThreadOwnership::OrangeDeck);
        assert_eq!(thread.status, CodexThreadStatus::WaitingApproval);
        assert_eq!(thread.project_id, Some(ProjectId::new("orange").unwrap()));
    }

    #[test]
    fn parses_stable_token_and_rate_limit_payloads() {
        let usage = parse_token_usage(&serde_json::json!({
            "tokenUsage": {
                "total": {
                    "inputTokens": 100,
                    "cachedInputTokens": 25,
                    "outputTokens": 20,
                    "reasoningOutputTokens": 5,
                    "totalTokens": 120
                },
                "modelContextWindow": 400_000
            }
        }))
        .unwrap();
        assert_eq!(usage.total_tokens, 120);
        assert_eq!(usage.model_context_window, Some(400_000));

        let limits = parse_limits(&serde_json::json!({
            "rateLimits": {
                "planType": "plus",
                "primary": { "usedPercent": 37, "resetsAt": 1234 },
                "secondary": null
            }
        }))
        .unwrap();
        assert_eq!(limits.primary.unwrap().remaining_percent(), 63);
    }

    #[test]
    fn approval_response_never_grants_session_wide_access() {
        let accepted = approval_result("item/commandExecution/requestApproval", true, &Value::Null);
        assert_eq!(accepted, serde_json::json!({ "decision": "accept" }));
        assert_ne!(accepted["decision"], "acceptForSession");

        let rejected = approval_result("item/fileChange/requestApproval", false, &Value::Null);
        assert_eq!(rejected, serde_json::json!({ "decision": "decline" }));
    }
}
