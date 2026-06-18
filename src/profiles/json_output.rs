use serde::Serialize;

use super::{Entry, StatusUsageJson};
use crate::PROFILE_ERR_SERIALIZE_INDEX;
use crate::json_response::JsonEnvelope;

#[derive(Serialize)]
struct ListedProfile {
    id: Option<String>,
    label: Option<String>,
    email: Option<String>,
    plan: Option<String>,
    is_current: bool,
    is_saved: bool,
    is_api_key: bool,
    managed_files: Vec<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ListedProfiles {
    profiles: Vec<ListedProfile>,
}

#[derive(Serialize)]
struct StatusProfileJson {
    id: Option<String>,
    label: Option<String>,
    email: Option<String>,
    plan: Option<String>,
    is_current: bool,
    is_saved: bool,
    is_api_key: bool,
    managed_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    warnings: Vec<String>,
    usage: Option<StatusUsageJson>,
    error: Option<StatusErrorJson>,
}

#[derive(Serialize)]
struct StatusErrorJson {
    summary: StatusErrorSummaryJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct StatusErrorSummaryJson {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct AllStatusJson {
    profiles: Vec<StatusProfileJson>,
}

pub(super) fn print_list_json(entries: &[Entry]) -> Result<(), String> {
    let profiles = entries
        .iter()
        .map(|entry| ListedProfile {
            id: entry.id.clone(),
            label: entry.label.clone(),
            email: entry.email.clone(),
            plan: entry.plan.clone(),
            is_current: entry.is_current,
            is_saved: entry.is_saved,
            is_api_key: entry.is_api_key,
            managed_files: entry.managed_files.clone(),
            error: entry.error_summary.clone(),
        })
        .collect();
    JsonEnvelope::success(
        "list",
        serde_json::to_value(ListedProfiles { profiles })
            .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?,
    )
    .print()
}

fn status_error_summary_json(summary: String) -> StatusErrorSummaryJson {
    let summary = crate::sanitize_for_terminal(&summary);
    let Some((start, end, response)) = extract_embedded_json_object(&summary) else {
        return StatusErrorSummaryJson {
            message: summary,
            response: None,
        };
    };

    StatusErrorSummaryJson {
        message: strip_embedded_json_segment(&summary, start, end),
        response: Some(response),
    }
}

fn extract_embedded_json_object(summary: &str) -> Option<(usize, usize, serde_json::Value)> {
    for (start, ch) in summary.char_indices() {
        if ch != '{' {
            continue;
        }
        let Some(end) = find_json_object_end(summary, start) else {
            continue;
        };
        let candidate = &summary[start..end];
        let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) else {
            continue;
        };
        return Some((start, end, value));
    }
    None
}

fn find_json_object_end(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in text[start..].char_indices() {
        let idx = start + offset;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}

fn strip_embedded_json_segment(text: &str, start: usize, end: usize) -> String {
    let left = text[..start].trim_end_matches([' ', ':']);
    let right = text[end..].trim_start_matches([',', ' ']);
    match (left.is_empty(), right.is_empty()) {
        (true, true) => String::new(),
        (true, false) => right.to_string(),
        (false, true) => left.to_string(),
        (false, false) => format!("{left}, {right}"),
    }
}

fn status_profile_json(entry: Entry) -> StatusProfileJson {
    let mut usage = entry.usage.map(|usage| StatusUsageJson {
        state: usage.state,
        buckets: usage.buckets,
        status_code: usage.status_code,
        summary: usage
            .summary
            .map(|summary| crate::sanitize_for_terminal(&summary)),
        detail: usage
            .detail
            .map(|detail| crate::sanitize_for_terminal(&detail)),
    });
    let mut top_level_summary = entry
        .error_summary
        .map(|error| crate::sanitize_for_terminal(&error));
    let mut error = None;
    if let Some(usage_json) = usage.as_mut()
        && usage_json.state == "error"
    {
        let status_code = usage_json.status_code.take();
        let detail = usage_json.detail.take();
        let usage_summary = usage_json.summary.take();
        let summary = top_level_summary.take().or(usage_summary);
        error = summary.map(|summary| StatusErrorJson {
            summary: status_error_summary_json(summary),
            status_code,
            detail,
        });
    }
    if error.is_none() {
        error = top_level_summary.map(|summary| StatusErrorJson {
            summary: status_error_summary_json(summary),
            status_code: None,
            detail: None,
        });
    }

    StatusProfileJson {
        id: entry.id,
        label: entry.label,
        email: entry.email,
        plan: entry.plan,
        is_current: entry.is_current,
        is_saved: entry.is_saved,
        is_api_key: entry.is_api_key,
        managed_files: entry.managed_files,
        warnings: entry
            .warnings
            .into_iter()
            .map(|warning| crate::sanitize_for_terminal(&warning))
            .collect(),
        usage,
        error,
    }
}

pub(super) fn print_current_status_json(current: Option<Entry>) -> Result<(), String> {
    let payload = current.map(status_profile_json);
    JsonEnvelope::success(
        "status",
        serde_json::to_value(payload)
            .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?,
    )
    .print()
}

pub(super) fn print_all_status_json(profiles: Vec<Entry>) -> Result<(), String> {
    let payload = AllStatusJson {
        profiles: profiles.into_iter().map(status_profile_json).collect(),
    };
    JsonEnvelope::success(
        "status",
        serde_json::to_value(payload)
            .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?,
    )
    .print()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_json_summary_is_split_into_message_and_response() {
        let summary = status_error_summary_json(
            "unexpected status 402: {\"detail\":{\"code\":\"deactivated\"}}".to_string(),
        );

        assert_eq!(summary.message, "unexpected status 402");
        assert_eq!(
            summary.response,
            Some(serde_json::json!({"detail": {"code": "deactivated"}}))
        );
    }

    #[test]
    fn plain_error_summary_remains_text() {
        let summary = status_error_summary_json("network unavailable".to_string());

        assert_eq!(summary.message, "network unavailable");
        assert!(summary.response.is_none());
    }
}
