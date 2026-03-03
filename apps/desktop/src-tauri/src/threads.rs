use provider_claude::{ClaudeAdapter, ClaudeThreadOverview, ClaudeThreadRuntimeState};
use provider_codex::{CodexAdapter, CodexThreadOverview, CodexThreadRuntimeState};
use provider_opencode::{OpenCodeAdapter, OpenCodeThreadOverview, OpenCodeThreadRuntimeState};
use std::collections::HashMap;

use crate::payloads::{
    ClaudeThreadRuntimeStatePayload, CodexThreadRuntimeStatePayload,
    OpenCodeThreadRuntimeStatePayload, ThreadSummaryPayload,
};

pub fn list_threads(project_path: Option<&str>) -> Result<Vec<ThreadSummaryPayload>, String> {
    let claude_threads = ClaudeAdapter::new()
        .list_thread_overviews(project_path)
        .map_err(|error| {
            format!(
                "Failed to list Claude threads ({:?}): {}",
                error.code, error.message
            )
        })?;
    let codex_threads = CodexAdapter::new()
        .list_thread_overviews(project_path)
        .map_err(|error| {
            format!(
                "Failed to list Codex threads ({:?}): {}",
                error.code, error.message
            )
        })?;
    let opencode_threads = OpenCodeAdapter::new()
        .list_thread_overviews(project_path)
        .map_err(|error| {
            format!(
                "Failed to list OpenCode threads ({:?}): {}",
                error.code, error.message
            )
        })?;

    let mut threads =
        Vec::with_capacity(claude_threads.len() + codex_threads.len() + opencode_threads.len());
    threads.extend(claude_threads.into_iter().map(map_claude_thread_overview));
    threads.extend(codex_threads.into_iter().map(map_codex_thread_overview));
    threads.extend(
        opencode_threads
            .into_iter()
            .map(map_opencode_thread_overview),
    );
    threads = dedupe_thread_summaries(threads);
    sort_thread_summaries(&mut threads);

    Ok(threads)
}

pub fn get_codex_thread_runtime_state(
    thread_id: &str,
) -> Result<CodexThreadRuntimeStatePayload, String> {
    let state = CodexAdapter::new()
        .get_thread_runtime_state(thread_id)
        .map_err(|error| {
            format!(
                "Failed to load Codex runtime state ({:?}): {}",
                error.code, error.message
            )
        })?;
    Ok(map_codex_thread_runtime_state(state))
}

pub fn get_claude_thread_runtime_state(
    thread_id: &str,
) -> Result<ClaudeThreadRuntimeStatePayload, String> {
    let state = ClaudeAdapter::new()
        .get_thread_runtime_state(thread_id)
        .map_err(|error| {
            format!(
                "Failed to load Claude runtime state ({:?}): {}",
                error.code, error.message
            )
        })?;
    Ok(map_claude_thread_runtime_state(state))
}

pub fn get_opencode_thread_runtime_state(
    thread_id: &str,
) -> Result<OpenCodeThreadRuntimeStatePayload, String> {
    let state = OpenCodeAdapter::new()
        .get_thread_runtime_state(thread_id)
        .map_err(|error| {
            format!(
                "Failed to load OpenCode runtime state ({:?}): {}",
                error.code, error.message
            )
        })?;
    Ok(map_opencode_thread_runtime_state(state))
}

fn map_claude_thread_overview(overview: ClaudeThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_codex_thread_overview(overview: CodexThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_opencode_thread_overview(overview: OpenCodeThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_codex_thread_runtime_state(
    state: CodexThreadRuntimeState,
) -> CodexThreadRuntimeStatePayload {
    CodexThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn map_opencode_thread_runtime_state(
    state: OpenCodeThreadRuntimeState,
) -> OpenCodeThreadRuntimeStatePayload {
    OpenCodeThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn map_claude_thread_runtime_state(
    state: ClaudeThreadRuntimeState,
) -> ClaudeThreadRuntimeStatePayload {
    ClaudeThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn sortable_last_active_at(raw: &str) -> i64 {
    let parsed = raw.parse::<i64>().unwrap_or(0);
    if parsed.abs() < 1_000_000_000_000 {
        parsed * 1000
    } else {
        parsed
    }
}

fn dedupe_thread_summaries(threads: Vec<ThreadSummaryPayload>) -> Vec<ThreadSummaryPayload> {
    let mut deduped: HashMap<(String, String), ThreadSummaryPayload> = HashMap::new();

    for thread in threads {
        let key = (thread.provider_id.clone(), thread.id.clone());
        match deduped.get(&key) {
            Some(existing) => {
                if should_replace_thread_summary(existing, &thread) {
                    deduped.insert(key, thread);
                }
            }
            None => {
                deduped.insert(key, thread);
            }
        }
    }

    deduped.into_values().collect()
}

fn should_replace_thread_summary(
    existing: &ThreadSummaryPayload,
    candidate: &ThreadSummaryPayload,
) -> bool {
    let existing_last_active = sortable_last_active_at(&existing.last_active_at);
    let candidate_last_active = sortable_last_active_at(&candidate.last_active_at);
    if candidate_last_active != existing_last_active {
        return candidate_last_active > existing_last_active;
    }

    candidate.project_path < existing.project_path
}

fn sort_thread_summaries(threads: &mut [ThreadSummaryPayload]) {
    threads.sort_by(|left, right| {
        sortable_last_active_at(&right.last_active_at)
            .cmp(&sortable_last_active_at(&left.last_active_at))
            .then_with(|| left.provider_id.cmp(&right.provider_id))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.project_path.cmp(&right.project_path))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_thread(
        provider_id: &str,
        id: &str,
        last_active_at: &str,
        project_path: &str,
    ) -> ThreadSummaryPayload {
        ThreadSummaryPayload {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            project_path: project_path.to_string(),
            title: format!("{provider_id}-{id}"),
            tags: vec![provider_id.to_string()],
            last_active_at: last_active_at.to_string(),
            last_message_preview: None,
        }
    }

    #[test]
    fn dedupe_thread_summaries_keeps_latest_record_for_same_provider_and_id() {
        let threads = vec![
            build_thread("claude_code", "session-1", "1700000000000", "/workspace/old"),
            build_thread("claude_code", "session-1", "1700000005000", "/workspace/new"),
            build_thread("codex", "session-1", "1700000001000", "/workspace/codex"),
        ];

        let mut deduped = dedupe_thread_summaries(threads);
        sort_thread_summaries(&mut deduped);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].provider_id, "claude_code");
        assert_eq!(deduped[0].id, "session-1");
        assert_eq!(deduped[0].project_path, "/workspace/new");
        assert_eq!(deduped[1].provider_id, "codex");
        assert_eq!(deduped[1].id, "session-1");
    }
}
