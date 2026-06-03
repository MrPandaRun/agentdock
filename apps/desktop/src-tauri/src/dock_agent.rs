use std::path::PathBuf;

use agentdock_core::dock_agent::{
    complete_remote_command, create_chat_connector, create_schedule, create_task,
    enqueue_due_schedules, get_chat_connector, get_remote_command, get_schedule,
    handle_chat_command, list_audit_logs, list_due_schedules, list_tasks_by_status,
    request_remote_command, start_remote_command,
};
use provider_contract::{
    DockAgentAuditLog, DockAgentChatCommandRequest, DockAgentChatCommandResult,
    DockAgentChatConnector, DockAgentRemoteCommand, DockAgentRemoteCommandDecision,
    DockAgentRemoteCommandRequest, DockAgentSchedule, DockAgentTask, DockAgentTaskStatus,
    ProviderId,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tauri::Manager;

use crate::payloads::{
    CompleteDockAgentRemoteCommandRequest, DockAgentAuditLogPayload,
    DockAgentChatCommandRequestPayload, DockAgentChatCommandResultPayload,
    DockAgentChatConnectorPayload, DockAgentDueScheduleEnqueuePayload,
    DockAgentRemoteCommandDecisionPayload, DockAgentRemoteCommandPayload,
    DockAgentRemoteCommandRequestPayload, DockAgentSchedulePayload, DockAgentTaskPayload,
    EnqueueDueDockAgentSchedulesRequest, GetDockAgentEntityRequest, ListDockAgentAuditLogsRequest,
    ListDockAgentTasksRequest, StartDockAgentRemoteCommandRequest,
};
use crate::provider_id::parse_provider_id;

#[derive(Debug, Clone)]
pub struct DockAgentContext {
    db_path: PathBuf,
}

impl DockAgentContext {
    pub fn from_app_handle(app: &tauri::AppHandle) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Failed to get app data directory: {error}"))?;
        Ok(Self {
            db_path: app_data_dir.join("agentdock.db"),
        })
    }

    fn get_connection(&self) -> Result<rusqlite::Connection, String> {
        agentdock_core::db::init_db(&self.db_path)
            .map_err(|error| format!("Failed to open Dock Agent database: {error}"))
    }
}

pub fn list_tasks_cmd(
    ctx: &DockAgentContext,
    request: ListDockAgentTasksRequest,
) -> Result<Vec<DockAgentTaskPayload>, String> {
    let conn = ctx.get_connection()?;
    let status = request
        .status
        .map(|value| enum_from_string::<DockAgentTaskStatus>(&value))
        .transpose()?;
    let tasks = list_tasks_by_status(&conn, status)
        .map_err(|error| format!("Failed to list Dock Agent tasks: {error}"))?;
    tasks.into_iter().map(task_to_payload).collect()
}

pub fn create_task_cmd(
    ctx: &DockAgentContext,
    request: DockAgentTaskPayload,
) -> Result<DockAgentTaskPayload, String> {
    let conn = ctx.get_connection()?;
    let task = task_from_payload(request)?;
    create_task(&conn, &task)
        .map_err(|error| format!("Failed to create Dock Agent task: {error}"))?;
    task_to_payload(task)
}

pub fn create_schedule_cmd(
    ctx: &DockAgentContext,
    request: DockAgentSchedulePayload,
) -> Result<DockAgentSchedulePayload, String> {
    let conn = ctx.get_connection()?;
    let schedule = schedule_from_payload(request)?;
    create_schedule(&conn, &schedule)
        .map_err(|error| format!("Failed to create Dock Agent schedule: {error}"))?;
    schedule_to_payload(schedule)
}

pub fn get_schedule_cmd(
    ctx: &DockAgentContext,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentSchedulePayload, String> {
    let conn = ctx.get_connection()?;
    let schedule = get_schedule(&conn, &request.id)
        .map_err(|error| format!("Failed to get Dock Agent schedule: {error}"))?;
    schedule_to_payload(schedule)
}

pub fn list_due_schedules_cmd(
    ctx: &DockAgentContext,
    now: String,
) -> Result<Vec<DockAgentSchedulePayload>, String> {
    let conn = ctx.get_connection()?;
    let schedules = list_due_schedules(&conn, &now)
        .map_err(|error| format!("Failed to list due Dock Agent schedules: {error}"))?;
    schedules.into_iter().map(schedule_to_payload).collect()
}

pub fn enqueue_due_schedules_cmd(
    ctx: &DockAgentContext,
    request: EnqueueDueDockAgentSchedulesRequest,
) -> Result<Vec<DockAgentDueScheduleEnqueuePayload>, String> {
    let mut conn = ctx.get_connection()?;
    let enqueued = enqueue_due_schedules(&mut conn, &request.now, &request.actor)
        .map_err(|error| format!("Failed to enqueue due Dock Agent schedules: {error}"))?;
    enqueued
        .into_iter()
        .map(|item| {
            Ok(DockAgentDueScheduleEnqueuePayload {
                schedule: schedule_to_payload(item.schedule)?,
                task: task_to_payload(item.task)?,
            })
        })
        .collect()
}

pub fn create_chat_connector_cmd(
    ctx: &DockAgentContext,
    request: DockAgentChatConnectorPayload,
) -> Result<DockAgentChatConnectorPayload, String> {
    let conn = ctx.get_connection()?;
    let connector = chat_connector_from_payload(request)?;
    create_chat_connector(&conn, &connector)
        .map_err(|error| format!("Failed to create Dock Agent chat connector: {error}"))?;
    chat_connector_to_payload(connector)
}

pub fn get_chat_connector_cmd(
    ctx: &DockAgentContext,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentChatConnectorPayload, String> {
    let conn = ctx.get_connection()?;
    let connector = get_chat_connector(&conn, &request.id)
        .map_err(|error| format!("Failed to get Dock Agent chat connector: {error}"))?;
    chat_connector_to_payload(connector)
}

pub fn handle_chat_command_cmd(
    ctx: &DockAgentContext,
    request: DockAgentChatCommandRequestPayload,
) -> Result<DockAgentChatCommandResultPayload, String> {
    let mut conn = ctx.get_connection()?;
    let request = DockAgentChatCommandRequest {
        connector_id: request.connector_id,
        external_message_id: request.external_message_id,
        actor: request.actor,
        text: request.text,
        received_at: request.received_at,
        raw_payload_json: request.raw_payload_json,
    };
    let result = handle_chat_command(&mut conn, &request)
        .map_err(|error| format!("Failed to handle Dock Agent chat command: {error}"))?;
    chat_command_result_to_payload(result)
}

pub fn request_remote_command_cmd(
    ctx: &DockAgentContext,
    request: DockAgentRemoteCommandRequestPayload,
) -> Result<DockAgentRemoteCommandDecisionPayload, String> {
    let mut conn = ctx.get_connection()?;
    let request = DockAgentRemoteCommandRequest {
        id: request.id,
        task_id: request.task_id,
        device_id: request.device_id,
        command: request.command,
        args_json: request.args_json,
        working_dir: request.working_dir,
        policy_json: request.policy_json,
        requested_by: request.requested_by,
        requested_at: request.requested_at,
    };
    let decision = request_remote_command(&mut conn, &request)
        .map_err(|error| format!("Failed to request Dock Agent remote command: {error}"))?;
    remote_command_decision_to_payload(decision)
}

pub fn get_remote_command_cmd(
    ctx: &DockAgentContext,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    let conn = ctx.get_connection()?;
    let command = get_remote_command(&conn, &request.id)
        .map_err(|error| format!("Failed to get Dock Agent remote command: {error}"))?;
    remote_command_to_payload(command)
}

pub fn start_remote_command_cmd(
    ctx: &DockAgentContext,
    request: StartDockAgentRemoteCommandRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    let mut conn = ctx.get_connection()?;
    let command = start_remote_command(&mut conn, &request.id, &request.actor, &request.started_at)
        .map_err(|error| format!("Failed to start Dock Agent remote command: {error}"))?;
    remote_command_to_payload(command)
}

pub fn complete_remote_command_cmd(
    ctx: &DockAgentContext,
    request: CompleteDockAgentRemoteCommandRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    let mut conn = ctx.get_connection()?;
    let command = complete_remote_command(
        &mut conn,
        &request.id,
        request.succeeded,
        &request.actor,
        &request.result_json,
        &request.completed_at,
    )
    .map_err(|error| format!("Failed to complete Dock Agent remote command: {error}"))?;
    remote_command_to_payload(command)
}

pub fn list_audit_logs_cmd(
    ctx: &DockAgentContext,
    request: ListDockAgentAuditLogsRequest,
) -> Result<Vec<DockAgentAuditLogPayload>, String> {
    let conn = ctx.get_connection()?;
    let logs = list_audit_logs(
        &conn,
        request.subject_type.as_deref(),
        request.subject_id.as_deref(),
    )
    .map_err(|error| format!("Failed to list Dock Agent audit logs: {error}"))?;
    logs.into_iter().map(audit_log_to_payload).collect()
}

fn task_from_payload(payload: DockAgentTaskPayload) -> Result<DockAgentTask, String> {
    Ok(DockAgentTask {
        id: payload.id,
        kind: enum_from_string(&payload.kind)?,
        status: enum_from_string(&payload.status)?,
        title: payload.title,
        objective: payload.objective,
        provider_id: payload
            .provider_id
            .as_deref()
            .map(parse_provider_id)
            .transpose()
            .map_err(|error| error.to_string())?,
        target_thread_id: payload.target_thread_id,
        schedule_id: payload.schedule_id,
        chat_connector_id: payload.chat_connector_id,
        remote_command_id: payload.remote_command_id,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
        started_at: payload.started_at,
        completed_at: payload.completed_at,
        metadata_json: payload.metadata_json,
    })
}

fn schedule_from_payload(payload: DockAgentSchedulePayload) -> Result<DockAgentSchedule, String> {
    Ok(DockAgentSchedule {
        id: payload.id,
        task_id: payload.task_id,
        cadence: enum_from_string(&payload.cadence)?,
        cron_expression: payload.cron_expression,
        timezone: payload.timezone,
        enabled: payload.enabled,
        next_run_at: payload.next_run_at,
        last_run_at: payload.last_run_at,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
    })
}

fn chat_connector_from_payload(
    payload: DockAgentChatConnectorPayload,
) -> Result<DockAgentChatConnector, String> {
    Ok(DockAgentChatConnector {
        id: payload.id,
        name: payload.name,
        kind: enum_from_string(&payload.kind)?,
        enabled: payload.enabled,
        config_json: payload.config_json,
        secret_ref: payload.secret_ref,
        last_seen_at: payload.last_seen_at,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
    })
}

fn task_to_payload(task: DockAgentTask) -> Result<DockAgentTaskPayload, String> {
    Ok(DockAgentTaskPayload {
        id: task.id,
        kind: enum_to_string(task.kind)?,
        status: enum_to_string(task.status)?,
        title: task.title,
        objective: task.objective,
        provider_id: task.provider_id.map(ProviderId::as_str).map(str::to_string),
        target_thread_id: task.target_thread_id,
        schedule_id: task.schedule_id,
        chat_connector_id: task.chat_connector_id,
        remote_command_id: task.remote_command_id,
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        metadata_json: task.metadata_json,
    })
}

fn schedule_to_payload(schedule: DockAgentSchedule) -> Result<DockAgentSchedulePayload, String> {
    Ok(DockAgentSchedulePayload {
        id: schedule.id,
        task_id: schedule.task_id,
        cadence: enum_to_string(schedule.cadence)?,
        cron_expression: schedule.cron_expression,
        timezone: schedule.timezone,
        enabled: schedule.enabled,
        next_run_at: schedule.next_run_at,
        last_run_at: schedule.last_run_at,
        created_at: schedule.created_at,
        updated_at: schedule.updated_at,
    })
}

fn chat_connector_to_payload(
    connector: DockAgentChatConnector,
) -> Result<DockAgentChatConnectorPayload, String> {
    Ok(DockAgentChatConnectorPayload {
        id: connector.id,
        name: connector.name,
        kind: enum_to_string(connector.kind)?,
        enabled: connector.enabled,
        config_json: connector.config_json,
        secret_ref: connector.secret_ref,
        last_seen_at: connector.last_seen_at,
        created_at: connector.created_at,
        updated_at: connector.updated_at,
    })
}

fn remote_command_to_payload(
    command: DockAgentRemoteCommand,
) -> Result<DockAgentRemoteCommandPayload, String> {
    Ok(DockAgentRemoteCommandPayload {
        id: command.id,
        task_id: command.task_id,
        device_id: command.device_id,
        status: enum_to_string(command.status)?,
        command: command.command,
        args_json: command.args_json,
        working_dir: command.working_dir,
        policy_json: command.policy_json,
        requested_by: command.requested_by,
        approved_at: command.approved_at,
        executed_at: command.executed_at,
        result_json: command.result_json,
        created_at: command.created_at,
        updated_at: command.updated_at,
    })
}

fn audit_log_to_payload(log: DockAgentAuditLog) -> Result<DockAgentAuditLogPayload, String> {
    Ok(DockAgentAuditLogPayload {
        id: log.id,
        actor: log.actor,
        action: log.action,
        subject_type: log.subject_type,
        subject_id: log.subject_id,
        outcome: enum_to_string(log.outcome)?,
        details_json: log.details_json,
        created_at: log.created_at,
    })
}

fn chat_command_result_to_payload(
    result: DockAgentChatCommandResult,
) -> Result<DockAgentChatCommandResultPayload, String> {
    Ok(DockAgentChatCommandResultPayload {
        accepted: result.accepted,
        task: result.task.map(task_to_payload).transpose()?,
        message: result.message,
    })
}

fn remote_command_decision_to_payload(
    decision: DockAgentRemoteCommandDecision,
) -> Result<DockAgentRemoteCommandDecisionPayload, String> {
    Ok(DockAgentRemoteCommandDecisionPayload {
        accepted: decision.accepted,
        command: remote_command_to_payload(decision.command)?,
        message: decision.message,
    })
}

fn enum_to_string<T>(value: T) -> Result<String, String>
where
    T: Serialize,
{
    match serde_json::to_value(value).map_err(|error| error.to_string())? {
        Value::String(value) => Ok(value),
        _ => Err("enum did not serialize to string".to_string()),
    }
}

fn enum_from_string<T>(value: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    serde_json::from_value(Value::String(value.to_string())).map_err(|error| error.to_string())
}
