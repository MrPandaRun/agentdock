use provider_claude::{
    ClaudeAdapter, ClaudeSendMessageResult, ClaudeThreadMessage, ClaudeThreadOverview,
    ClaudeThreadRuntimeState,
};
use provider_codex::{
    CodexAdapter, CodexThreadMessage, CodexThreadOverview, CodexThreadRuntimeState,
};
use provider_contract::ProviderId;
use provider_opencode::{
    OpenCodeAdapter, OpenCodeThreadMessage, OpenCodeThreadOverview, OpenCodeThreadRuntimeState,
};

use crate::payloads::{
    ClaudeThreadRuntimeStatePayload, CodexThreadRuntimeStatePayload,
    OpenCodeThreadRuntimeStatePayload, SendClaudeMessageResponse, ThreadMessagePayload,
    ThreadSummaryPayload,
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
    threads
        .sort_by_key(|thread| std::cmp::Reverse(sortable_last_active_at(&thread.last_active_at)));

    Ok(threads)
}

pub fn get_thread_messages(
    provider_id: ProviderId,
    thread_id: &str,
) -> Result<Vec<ThreadMessagePayload>, String> {
    match provider_id {
        ProviderId::ClaudeCode => {
            let messages = ClaudeAdapter::new()
                .get_thread_messages(thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load Claude thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_claude_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
        ProviderId::Codex => {
            let messages = CodexAdapter::new()
                .get_thread_messages(thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load Codex thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_codex_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
        ProviderId::OpenCode => {
            let messages = OpenCodeAdapter::new()
                .get_thread_messages(thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load OpenCode thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_opencode_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
    }
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

pub fn send_claude_message(
    thread_id: &str,
    content: &str,
    project_path: Option<&str>,
) -> Result<SendClaudeMessageResponse, String> {
    let adapter = ClaudeAdapter::new();
    let result = adapter
        .send_message(thread_id, content, project_path)
        .map_err(|error| {
            format!(
                "Failed to send message to Claude thread ({:?}): {}",
                error.code, error.message
            )
        })?;
    Ok(map_send_message_result(result))
}

fn map_send_message_result(result: ClaudeSendMessageResult) -> SendClaudeMessageResponse {
    SendClaudeMessageResponse {
        thread_id: result.thread_id,
        response_text: result.response_text,
        raw_output: result.raw_output,
    }
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

fn map_claude_thread_message(message: ClaudeThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
    }
}

fn map_codex_thread_message(message: CodexThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
    }
}

fn map_opencode_thread_message(message: OpenCodeThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
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
