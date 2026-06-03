use chrono::{DateTime, Duration, Utc};
use provider_contract::{
    DockAgentAuditLog, DockAgentAuditOutcome, DockAgentChatCommandRequest,
    DockAgentChatCommandResult, DockAgentChatConnector, DockAgentChatConnectorKind,
    DockAgentRemoteCommand, DockAgentRemoteCommandDecision, DockAgentRemoteCommandRequest,
    DockAgentRemoteCommandStatus, DockAgentSchedule, DockAgentScheduleCadence, DockAgentTask,
    DockAgentTaskKind, DockAgentTaskStatus, ProviderId,
};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DockAgentError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("time parse error: {0}")]
    Time(#[from] chrono::ParseError),
    #[error("dock agent task not found: {0}")]
    TaskNotFound(String),
    #[error("dock agent schedule not found: {0}")]
    ScheduleNotFound(String),
    #[error("dock agent chat connector not found: {0}")]
    ChatConnectorNotFound(String),
    #[error("dock agent remote command not found: {0}")]
    RemoteCommandNotFound(String),
    #[error("invalid remote command status transition from {from:?} to {to:?}")]
    InvalidRemoteCommandTransition {
        from: DockAgentRemoteCommandStatus,
        to: DockAgentRemoteCommandStatus,
    },
    #[error("validation error: {0}")]
    Validation(String),
    #[error("invalid task status transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: DockAgentTaskStatus,
        to: DockAgentTaskStatus,
    },
}

pub fn create_task(connection: &Connection, task: &DockAgentTask) -> Result<(), DockAgentError> {
    validate_task(task)?;

    insert_task(connection, task, "INSERT INTO")?;

    Ok(())
}

fn insert_task(
    connection: &Connection,
    task: &DockAgentTask,
    insert_clause: &str,
) -> Result<usize, DockAgentError> {
    let provider_id = task.provider_id.map(ProviderId::as_str);
    let kind = enum_to_db_value(task.kind)?;
    let status = enum_to_db_value(task.status)?;

    let sql = format!(
        "{insert_clause} dock_agent_tasks (
            id, kind, status, title, objective, provider_id, target_thread_id,
            schedule_id, chat_connector_id, remote_command_id, metadata_json,
            created_at, updated_at, started_at, completed_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
            ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15
         )"
    );

    let rows_affected = connection.execute(
        &sql,
        params![
            task.id,
            kind,
            status,
            task.title,
            task.objective,
            provider_id,
            task.target_thread_id,
            task.schedule_id,
            task.chat_connector_id,
            task.remote_command_id,
            task.metadata_json,
            task.created_at,
            task.updated_at,
            task.started_at,
            task.completed_at,
        ],
    )?;

    Ok(rows_affected)
}

pub fn get_task(connection: &Connection, id: &str) -> Result<DockAgentTask, DockAgentError> {
    get_task_from_connection(connection, id)
}

pub fn list_tasks_by_status(
    connection: &Connection,
    status: Option<DockAgentTaskStatus>,
) -> Result<Vec<DockAgentTask>, DockAgentError> {
    let mut query = String::from(
        "SELECT id, kind, status, title, objective, provider_id, target_thread_id,
                schedule_id, chat_connector_id, remote_command_id,
                created_at, updated_at, started_at, completed_at, metadata_json
         FROM dock_agent_tasks",
    );
    let status_value = match status {
        Some(value) => {
            query.push_str(" WHERE status = ?1");
            Some(enum_to_db_value(value)?)
        }
        None => None,
    };
    query.push_str(" ORDER BY updated_at DESC, created_at DESC, id ASC");

    let mut stmt = connection.prepare(&query)?;
    let tasks = match status_value {
        Some(value) => stmt
            .query_map(params![value], map_task_row)?
            .collect::<Result<Vec<_>, _>>()?,
        None => stmt
            .query_map([], map_task_row)?
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(tasks)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueScheduleEnqueueResult {
    pub schedule: DockAgentSchedule,
    pub task: DockAgentTask,
}

pub fn create_schedule(
    connection: &Connection,
    schedule: &DockAgentSchedule,
) -> Result<(), DockAgentError> {
    validate_schedule(schedule)?;

    let cadence = enum_to_db_value(schedule.cadence)?;
    connection.execute(
        "INSERT INTO dock_agent_schedules (
            id, task_id, cadence, cron_expression, timezone, enabled,
            next_run_at, last_run_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            schedule.id,
            schedule.task_id,
            cadence,
            schedule.cron_expression,
            schedule.timezone,
            if schedule.enabled { 1_i64 } else { 0_i64 },
            schedule.next_run_at,
            schedule.last_run_at,
            schedule.created_at,
            schedule.updated_at,
        ],
    )?;

    Ok(())
}

pub fn get_schedule(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentSchedule, DockAgentError> {
    get_schedule_from_connection(connection, id)
}

pub fn list_due_schedules(
    connection: &Connection,
    now: &str,
) -> Result<Vec<DockAgentSchedule>, DockAgentError> {
    let now = parse_timestamp(now)?;
    let mut stmt = connection.prepare(
        "SELECT id, task_id, cadence, cron_expression, timezone, enabled,
                next_run_at, last_run_at, created_at, updated_at
         FROM dock_agent_schedules
         WHERE enabled = 1 AND next_run_at IS NOT NULL
         ORDER BY next_run_at ASC, id ASC",
    )?;
    let schedules = stmt
        .query_map([], map_schedule_row)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|schedule| {
            schedule
                .next_run_at
                .as_deref()
                .and_then(|value| parse_timestamp(value).ok())
                .is_some_and(|next_run_at| next_run_at <= now)
        })
        .collect();

    Ok(schedules)
}

pub fn enqueue_due_schedules(
    connection: &mut Connection,
    now: &str,
    actor: &str,
) -> Result<Vec<DueScheduleEnqueueResult>, DockAgentError> {
    if actor.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "audit actor is required for scheduler runs".to_string(),
        ));
    }

    let now_timestamp = parse_timestamp(now)?;
    let transaction = connection.transaction()?;
    let due_schedules = list_due_schedules(&transaction, now)?;
    let mut enqueued = Vec::new();

    for schedule in due_schedules {
        let due_at = schedule.next_run_at.as_deref().ok_or_else(|| {
            DockAgentError::Validation("due schedule is missing next_run_at".to_string())
        })?;
        let task = build_scheduled_task(&transaction, &schedule, due_at, now)?;
        let rows_affected = insert_task(&transaction, &task, "INSERT OR IGNORE INTO")?;
        let next_run_at = next_run_at_after(&schedule, due_at)?;

        transaction.execute(
            "UPDATE dock_agent_schedules
             SET last_run_at = ?1,
                 next_run_at = ?2,
                 updated_at = ?3
             WHERE id = ?4",
            params![due_at, next_run_at, now, schedule.id],
        )?;

        let audit_details = json!({
            "dueAt": due_at,
            "taskId": task.id,
            "inserted": rows_affected > 0,
            "schedulerNow": now_timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        })
        .to_string();
        insert_audit_log_in_connection(
            &transaction,
            actor,
            "schedule.enqueue_due",
            "dock_agent_schedule",
            Some(&schedule.id),
            DockAgentAuditOutcome::Succeeded,
            &audit_details,
        )?;

        if rows_affected > 0 {
            enqueued.push(DueScheduleEnqueueResult { schedule, task });
        }
    }

    transaction.commit()?;
    Ok(enqueued)
}

pub fn create_chat_connector(
    connection: &Connection,
    connector: &DockAgentChatConnector,
) -> Result<(), DockAgentError> {
    validate_chat_connector(connector)?;

    let kind = enum_to_db_value(connector.kind)?;
    connection.execute(
        "INSERT INTO dock_agent_chat_connectors (
            id, name, kind, enabled, config_json, secret_ref, last_seen_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            connector.id,
            connector.name,
            kind,
            if connector.enabled { 1_i64 } else { 0_i64 },
            connector.config_json,
            connector.secret_ref,
            connector.last_seen_at,
            connector.created_at,
            connector.updated_at,
        ],
    )?;

    Ok(())
}

pub fn get_chat_connector(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentChatConnector, DockAgentError> {
    get_chat_connector_from_connection(connection, id)
}

pub fn handle_chat_command(
    connection: &mut Connection,
    request: &DockAgentChatCommandRequest,
) -> Result<DockAgentChatCommandResult, DockAgentError> {
    validate_chat_command_request(request)?;
    let transaction = connection.transaction()?;
    let connector = match get_chat_connector_from_connection(&transaction, &request.connector_id) {
        Ok(connector) => connector,
        Err(DockAgentError::ChatConnectorNotFound(_)) => {
            let details = json!({
                "reason": "connector_not_found",
                "actor": request.actor,
                "externalMessageId": request.external_message_id,
            })
            .to_string();
            insert_audit_log_in_connection(
                &transaction,
                &request.actor,
                "chat.command_rejected",
                "dock_agent_chat_connector",
                Some(&request.connector_id),
                DockAgentAuditOutcome::Denied,
                &details,
            )?;
            transaction.commit()?;
            return Ok(DockAgentChatCommandResult {
                accepted: false,
                task: None,
                message: Some("Chat connector is not registered.".to_string()),
            });
        }
        Err(error) => return Err(error),
    };

    if !connector.enabled {
        let details = json!({
            "reason": "connector_disabled",
            "actor": request.actor,
            "externalMessageId": request.external_message_id,
        })
        .to_string();
        insert_audit_log_in_connection(
            &transaction,
            &request.actor,
            "chat.command_rejected",
            "dock_agent_chat_connector",
            Some(&connector.id),
            DockAgentAuditOutcome::Denied,
            &details,
        )?;
        transaction.commit()?;
        return Ok(DockAgentChatCommandResult {
            accepted: false,
            task: None,
            message: Some("Chat connector is disabled.".to_string()),
        });
    }

    let task = build_chat_command_task(&connector, request)?;
    let rows_affected = insert_task(&transaction, &task, "INSERT OR IGNORE INTO")?;
    let audit_details = json!({
        "actor": request.actor,
        "externalMessageId": request.external_message_id,
        "taskId": task.id,
        "inserted": rows_affected > 0,
    })
    .to_string();
    insert_audit_log_in_connection(
        &transaction,
        &request.actor,
        "chat.command_accepted",
        "dock_agent_chat_connector",
        Some(&connector.id),
        DockAgentAuditOutcome::Allowed,
        &audit_details,
    )?;
    transaction.execute(
        "UPDATE dock_agent_chat_connectors
         SET last_seen_at = ?1,
             updated_at = ?1
         WHERE id = ?2",
        params![request.received_at, connector.id],
    )?;
    transaction.commit()?;

    Ok(DockAgentChatCommandResult {
        accepted: true,
        task: if rows_affected > 0 { Some(task) } else { None },
        message: if rows_affected > 0 {
            Some("Chat command accepted.".to_string())
        } else {
            Some("Chat command was already accepted.".to_string())
        },
    })
}

pub fn request_remote_command(
    connection: &mut Connection,
    request: &DockAgentRemoteCommandRequest,
) -> Result<DockAgentRemoteCommandDecision, DockAgentError> {
    validate_remote_command_request(request)?;

    let transaction = connection.transaction()?;
    let policy_decision = evaluate_remote_command_policy(request)?;
    let status = if policy_decision.allowed {
        DockAgentRemoteCommandStatus::Approved
    } else {
        DockAgentRemoteCommandStatus::Rejected
    };
    let mut command = DockAgentRemoteCommand {
        id: request.id.clone(),
        task_id: request.task_id.clone(),
        device_id: request.device_id.clone(),
        status,
        command: request.command.trim().to_string(),
        args_json: request.args_json.clone(),
        working_dir: request.working_dir.clone(),
        policy_json: request.policy_json.clone(),
        requested_by: request.requested_by.clone(),
        approved_at: if status == DockAgentRemoteCommandStatus::Approved {
            Some(request.requested_at.clone())
        } else {
            None
        },
        executed_at: None,
        result_json: if policy_decision.allowed {
            "{}".to_string()
        } else {
            json!({ "reason": policy_decision.message }).to_string()
        },
        created_at: request.requested_at.clone(),
        updated_at: request.requested_at.clone(),
    };

    insert_remote_command(&transaction, &command, "INSERT INTO")?;
    if status == DockAgentRemoteCommandStatus::Approved {
        let task = build_remote_command_task(&command)?;
        insert_task(&transaction, &task, "INSERT OR IGNORE INTO")?;
        command.task_id = Some(task.id.clone());
        transaction.execute(
            "UPDATE dock_agent_remote_commands
             SET task_id = ?1
             WHERE id = ?2",
            params![task.id, command.id],
        )?;
    }

    let action = if policy_decision.allowed {
        "remote_command.approved"
    } else {
        "remote_command.rejected"
    };
    let outcome = if policy_decision.allowed {
        DockAgentAuditOutcome::Allowed
    } else {
        DockAgentAuditOutcome::Denied
    };
    let audit_details = json!({
        "command": command.command,
        "args": parse_json_value(&command.args_json, "args_json")?,
        "workingDir": command.working_dir,
        "message": policy_decision.message,
    })
    .to_string();
    insert_audit_log_in_connection(
        &transaction,
        &request.requested_by,
        action,
        "dock_agent_remote_command",
        Some(&command.id),
        outcome,
        &audit_details,
    )?;

    let persisted = get_remote_command_from_connection(&transaction, &command.id)?;
    transaction.commit()?;
    Ok(DockAgentRemoteCommandDecision {
        accepted: status == DockAgentRemoteCommandStatus::Approved,
        command: persisted,
        message: Some(policy_decision.message),
    })
}

pub fn get_remote_command(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentRemoteCommand, DockAgentError> {
    get_remote_command_from_connection(connection, id)
}

pub fn start_remote_command(
    connection: &mut Connection,
    id: &str,
    actor: &str,
    started_at: &str,
) -> Result<DockAgentRemoteCommand, DockAgentError> {
    transition_remote_command(
        connection,
        id,
        DockAgentRemoteCommandStatus::Running,
        actor,
        "{}",
        started_at,
        None,
    )
}

pub fn complete_remote_command(
    connection: &mut Connection,
    id: &str,
    succeeded: bool,
    actor: &str,
    result_json: &str,
    completed_at: &str,
) -> Result<DockAgentRemoteCommand, DockAgentError> {
    validate_json(result_json, "result_json")?;
    let next_status = if succeeded {
        DockAgentRemoteCommandStatus::Succeeded
    } else {
        DockAgentRemoteCommandStatus::Failed
    };
    transition_remote_command(
        connection,
        id,
        next_status,
        actor,
        result_json,
        completed_at,
        Some(result_json),
    )
}

pub fn transition_task(
    connection: &mut Connection,
    id: &str,
    next_status: DockAgentTaskStatus,
    actor: &str,
    details_json: &str,
    changed_at: &str,
) -> Result<DockAgentTask, DockAgentError> {
    if actor.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "audit actor is required for task transitions".to_string(),
        ));
    }
    validate_json(details_json, "details_json")?;

    let transaction = connection.transaction()?;
    let current = get_task_from_connection(&transaction, id)?;
    validate_transition(current.status, next_status)?;

    let next_status_value = enum_to_db_value(next_status)?;
    let started_at = if next_status == DockAgentTaskStatus::Running && current.started_at.is_none()
    {
        Some(changed_at)
    } else {
        current.started_at.as_deref()
    };
    let completed_at = if is_terminal_status(next_status) {
        Some(changed_at)
    } else {
        current.completed_at.as_deref()
    };

    transaction.execute(
        "UPDATE dock_agent_tasks
         SET status = ?1,
             updated_at = ?2,
             started_at = ?3,
             completed_at = ?4
         WHERE id = ?5",
        params![next_status_value, changed_at, started_at, completed_at, id],
    )?;

    insert_audit_log_in_connection(
        &transaction,
        actor,
        "task.transition",
        "dock_agent_task",
        Some(id),
        DockAgentAuditOutcome::Succeeded,
        details_json,
    )?;

    let updated = get_task_from_connection(&transaction, id)?;
    transaction.commit()?;
    Ok(updated)
}

pub fn list_audit_logs(
    connection: &Connection,
    subject_type: Option<&str>,
    subject_id: Option<&str>,
) -> Result<Vec<DockAgentAuditLog>, DockAgentError> {
    let (query, params): (&str, Vec<&str>) = match (subject_type, subject_id) {
        (Some(kind), Some(id)) => (
            "SELECT id, actor, action, subject_type, subject_id, outcome, details_json, created_at
             FROM dock_agent_audit_logs
             WHERE subject_type = ?1 AND subject_id = ?2
             ORDER BY created_at DESC, id DESC",
            vec![kind, id],
        ),
        (Some(kind), None) => (
            "SELECT id, actor, action, subject_type, subject_id, outcome, details_json, created_at
             FROM dock_agent_audit_logs
             WHERE subject_type = ?1
             ORDER BY created_at DESC, id DESC",
            vec![kind],
        ),
        (None, Some(id)) => (
            "SELECT id, actor, action, subject_type, subject_id, outcome, details_json, created_at
             FROM dock_agent_audit_logs
             WHERE subject_id = ?1
             ORDER BY created_at DESC, id DESC",
            vec![id],
        ),
        (None, None) => (
            "SELECT id, actor, action, subject_type, subject_id, outcome, details_json, created_at
             FROM dock_agent_audit_logs
             ORDER BY created_at DESC, id DESC",
            Vec::new(),
        ),
    };

    let mut stmt = connection.prepare(query)?;
    let logs = stmt
        .query_map(rusqlite::params_from_iter(params), map_audit_log_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(logs)
}

fn get_task_from_connection(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentTask, DockAgentError> {
    connection
        .query_row(
            "SELECT id, kind, status, title, objective, provider_id, target_thread_id,
                    schedule_id, chat_connector_id, remote_command_id,
                    created_at, updated_at, started_at, completed_at, metadata_json
             FROM dock_agent_tasks
             WHERE id = ?1",
            params![id],
            map_task_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => DockAgentError::TaskNotFound(id.to_string()),
            other => DockAgentError::Sqlite(other),
        })
}

fn get_schedule_from_connection(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentSchedule, DockAgentError> {
    connection
        .query_row(
            "SELECT id, task_id, cadence, cron_expression, timezone, enabled,
                    next_run_at, last_run_at, created_at, updated_at
             FROM dock_agent_schedules
             WHERE id = ?1",
            params![id],
            map_schedule_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                DockAgentError::ScheduleNotFound(id.to_string())
            }
            other => DockAgentError::Sqlite(other),
        })
}

fn get_chat_connector_from_connection(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentChatConnector, DockAgentError> {
    connection
        .query_row(
            "SELECT id, name, kind, enabled, config_json, secret_ref,
                    last_seen_at, created_at, updated_at
             FROM dock_agent_chat_connectors
             WHERE id = ?1",
            params![id],
            map_chat_connector_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                DockAgentError::ChatConnectorNotFound(id.to_string())
            }
            other => DockAgentError::Sqlite(other),
        })
}

fn get_remote_command_from_connection(
    connection: &Connection,
    id: &str,
) -> Result<DockAgentRemoteCommand, DockAgentError> {
    connection
        .query_row(
            "SELECT id, task_id, device_id, status, command, args_json, working_dir,
                    policy_json, requested_by, approved_at, executed_at, result_json,
                    created_at, updated_at
             FROM dock_agent_remote_commands
             WHERE id = ?1",
            params![id],
            map_remote_command_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                DockAgentError::RemoteCommandNotFound(id.to_string())
            }
            other => DockAgentError::Sqlite(other),
        })
}

fn insert_remote_command(
    connection: &Connection,
    command: &DockAgentRemoteCommand,
    insert_clause: &str,
) -> Result<usize, DockAgentError> {
    validate_remote_command(command)?;
    let status = enum_to_db_value(command.status)?;
    let sql = format!(
        "{insert_clause} dock_agent_remote_commands (
            id, task_id, device_id, status, command, args_json, working_dir,
            policy_json, requested_by, approved_at, executed_at, result_json,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
    );
    let rows_affected = connection.execute(
        &sql,
        params![
            command.id,
            command.task_id,
            command.device_id,
            status,
            command.command,
            command.args_json,
            command.working_dir,
            command.policy_json,
            command.requested_by,
            command.approved_at,
            command.executed_at,
            command.result_json,
            command.created_at,
            command.updated_at,
        ],
    )?;
    Ok(rows_affected)
}

fn insert_audit_log_in_connection(
    connection: &Connection,
    actor: &str,
    action: &str,
    subject_type: &str,
    subject_id: Option<&str>,
    outcome: DockAgentAuditOutcome,
    details_json: &str,
) -> Result<(), DockAgentError> {
    if actor.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "audit actor is required".to_string(),
        ));
    }
    validate_json(details_json, "details_json")?;

    let outcome = enum_to_db_value(outcome)?;
    connection.execute(
        "INSERT INTO dock_agent_audit_logs (
            actor, action, subject_type, subject_id, outcome, details_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            actor,
            action,
            subject_type,
            subject_id,
            outcome,
            details_json
        ],
    )?;

    Ok(())
}

fn validate_task(task: &DockAgentTask) -> Result<(), DockAgentError> {
    if task.id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "task id is required".to_string(),
        ));
    }
    if task.title.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "task title is required".to_string(),
        ));
    }
    if task.objective.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "task objective is required".to_string(),
        ));
    }
    if let Some(provider_id) = task.provider_id {
        validate_mvp_provider(provider_id)?;
    }
    validate_json(&task.metadata_json, "metadata_json")?;
    Ok(())
}

fn validate_schedule(schedule: &DockAgentSchedule) -> Result<(), DockAgentError> {
    if schedule.id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "schedule id is required".to_string(),
        ));
    }
    if schedule.timezone.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "schedule timezone is required".to_string(),
        ));
    }
    if matches!(schedule.cadence, DockAgentScheduleCadence::Cron)
        && schedule
            .cron_expression
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(DockAgentError::Validation(
            "cron schedules require cron_expression".to_string(),
        ));
    }
    if let Some(next_run_at) = schedule.next_run_at.as_deref() {
        parse_timestamp(next_run_at)?;
    }
    if let Some(last_run_at) = schedule.last_run_at.as_deref() {
        parse_timestamp(last_run_at)?;
    }
    Ok(())
}

fn validate_chat_connector(connector: &DockAgentChatConnector) -> Result<(), DockAgentError> {
    if connector.id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "chat connector id is required".to_string(),
        ));
    }
    if connector.name.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "chat connector name is required".to_string(),
        ));
    }
    if !matches!(connector.kind, DockAgentChatConnectorKind::Webhook) {
        return Err(DockAgentError::Validation(
            "only webhook chat connectors are supported".to_string(),
        ));
    }
    let config = parse_json_value(&connector.config_json, "config_json")?;
    if contains_secret_like_key(&config) {
        return Err(DockAgentError::Validation(
            "chat connector config_json must not contain secret-like keys; use secret_ref"
                .to_string(),
        ));
    }
    if let Some(last_seen_at) = connector.last_seen_at.as_deref() {
        parse_timestamp(last_seen_at)?;
    }
    Ok(())
}

fn validate_chat_command_request(
    request: &DockAgentChatCommandRequest,
) -> Result<(), DockAgentError> {
    if request.connector_id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "chat connector id is required".to_string(),
        ));
    }
    if request.actor.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "chat command actor is required".to_string(),
        ));
    }
    if request.text.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "chat command text is required".to_string(),
        ));
    }
    parse_timestamp(&request.received_at)?;
    validate_json(&request.raw_payload_json, "raw_payload_json")?;
    Ok(())
}

fn validate_remote_command_request(
    request: &DockAgentRemoteCommandRequest,
) -> Result<(), DockAgentError> {
    if request.id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command id is required".to_string(),
        ));
    }
    if request.command.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command is required".to_string(),
        ));
    }
    if request.requested_by.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command requester is required".to_string(),
        ));
    }
    parse_timestamp(&request.requested_at)?;
    validate_args_json(&request.args_json)?;
    validate_json(&request.policy_json, "policy_json")?;
    Ok(())
}

fn validate_remote_command(command: &DockAgentRemoteCommand) -> Result<(), DockAgentError> {
    if command.id.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command id is required".to_string(),
        ));
    }
    if command.command.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command is required".to_string(),
        ));
    }
    if command.requested_by.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "remote command requester is required".to_string(),
        ));
    }
    validate_args_json(&command.args_json)?;
    validate_json(&command.policy_json, "policy_json")?;
    validate_json(&command.result_json, "result_json")?;
    Ok(())
}

fn validate_args_json(raw: &str) -> Result<(), DockAgentError> {
    let value = parse_json_value(raw, "args_json")?;
    let args = value.as_array().ok_or_else(|| {
        DockAgentError::Validation("args_json must contain an array of strings".to_string())
    })?;
    if args.iter().any(|value| !value.is_string()) {
        return Err(DockAgentError::Validation(
            "args_json must contain an array of strings".to_string(),
        ));
    }
    Ok(())
}

fn validate_mvp_provider(provider_id: ProviderId) -> Result<(), DockAgentError> {
    match provider_id {
        ProviderId::Codex | ProviderId::ClaudeCode | ProviderId::OpenCode => Ok(()),
    }
}

fn validate_transition(
    current: DockAgentTaskStatus,
    next: DockAgentTaskStatus,
) -> Result<(), DockAgentError> {
    let allowed = matches!(
        (current, next),
        (DockAgentTaskStatus::Queued, DockAgentTaskStatus::Running)
            | (DockAgentTaskStatus::Queued, DockAgentTaskStatus::Canceled)
            | (DockAgentTaskStatus::Running, DockAgentTaskStatus::Succeeded)
            | (DockAgentTaskStatus::Running, DockAgentTaskStatus::Failed)
            | (DockAgentTaskStatus::Running, DockAgentTaskStatus::Canceled)
    );

    if allowed {
        Ok(())
    } else {
        Err(DockAgentError::InvalidTransition {
            from: current,
            to: next,
        })
    }
}

fn is_terminal_status(status: DockAgentTaskStatus) -> bool {
    matches!(
        status,
        DockAgentTaskStatus::Succeeded
            | DockAgentTaskStatus::Failed
            | DockAgentTaskStatus::Canceled
    )
}

fn validate_json(raw: &str, field_name: &str) -> Result<(), DockAgentError> {
    parse_json_value(raw, field_name).map(|_| ())
}

fn parse_json_value(raw: &str, field_name: &str) -> Result<Value, DockAgentError> {
    serde_json::from_str::<Value>(raw).map_err(|error| {
        DockAgentError::Validation(format!("{field_name} must contain valid JSON: {error}"))
    })
}

fn contains_secret_like_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = key.to_ascii_lowercase();
            normalized.contains("secret")
                || normalized.contains("token")
                || normalized.contains("password")
                || normalized.contains("apikey")
                || normalized.contains("api_key")
                || contains_secret_like_key(value)
        }),
        Value::Array(values) => values.iter().any(contains_secret_like_key),
        _ => false,
    }
}

fn build_scheduled_task(
    connection: &Connection,
    schedule: &DockAgentSchedule,
    due_at: &str,
    now: &str,
) -> Result<DockAgentTask, DockAgentError> {
    let task_id = scheduled_task_id(&schedule.id, due_at);
    let template = schedule
        .task_id
        .as_deref()
        .map(|id| get_task_from_connection(connection, id))
        .transpose()?;
    let metadata_json = json!({
        "scheduleId": schedule.id,
        "dueAt": due_at,
        "templateTaskId": schedule.task_id,
    })
    .to_string();

    Ok(DockAgentTask {
        id: task_id,
        kind: DockAgentTaskKind::ScheduledCheck,
        status: DockAgentTaskStatus::Queued,
        title: template
            .as_ref()
            .map(|task| task.title.clone())
            .unwrap_or_else(|| format!("Scheduled check: {}", schedule.id)),
        objective: template
            .as_ref()
            .map(|task| task.objective.clone())
            .unwrap_or_else(|| {
                format!(
                    "Run scheduled Dock Agent check for schedule {}.",
                    schedule.id
                )
            }),
        provider_id: template.as_ref().and_then(|task| task.provider_id),
        target_thread_id: template
            .as_ref()
            .and_then(|task| task.target_thread_id.clone()),
        schedule_id: Some(schedule.id.clone()),
        chat_connector_id: None,
        remote_command_id: None,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        started_at: None,
        completed_at: None,
        metadata_json,
    })
}

fn build_chat_command_task(
    connector: &DockAgentChatConnector,
    request: &DockAgentChatCommandRequest,
) -> Result<DockAgentTask, DockAgentError> {
    let task_id = chat_task_id(request);
    let metadata_json = json!({
        "connectorId": connector.id,
        "connectorKind": enum_to_db_value(connector.kind)?,
        "externalMessageId": request.external_message_id,
        "actor": request.actor,
        "receivedAt": request.received_at,
        "rawPayload": parse_json_value(&request.raw_payload_json, "raw_payload_json")?,
    })
    .to_string();

    Ok(DockAgentTask {
        id: task_id,
        kind: DockAgentTaskKind::ChatCommand,
        status: DockAgentTaskStatus::Queued,
        title: format!("Chat command from {}", request.actor),
        objective: request.text.trim().to_string(),
        provider_id: None,
        target_thread_id: None,
        schedule_id: None,
        chat_connector_id: Some(connector.id.clone()),
        remote_command_id: None,
        created_at: request.received_at.clone(),
        updated_at: request.received_at.clone(),
        started_at: None,
        completed_at: None,
        metadata_json,
    })
}

fn build_remote_command_task(
    command: &DockAgentRemoteCommand,
) -> Result<DockAgentTask, DockAgentError> {
    Ok(DockAgentTask {
        id: format!("remote-command-{}", sanitize_id_segment(&command.id)),
        kind: DockAgentTaskKind::RemoteCommand,
        status: DockAgentTaskStatus::Queued,
        title: format!("Remote command: {}", command.command),
        objective: format!(
            "Run approved remote command `{}` for {}.",
            command.command, command.requested_by
        ),
        provider_id: None,
        target_thread_id: None,
        schedule_id: None,
        chat_connector_id: None,
        remote_command_id: Some(command.id.clone()),
        created_at: command.created_at.clone(),
        updated_at: command.updated_at.clone(),
        started_at: None,
        completed_at: None,
        metadata_json: json!({
            "command": command.command,
            "args": parse_json_value(&command.args_json, "args_json")?,
            "workingDir": command.working_dir,
            "deviceId": command.device_id,
        })
        .to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteCommandPolicyDecision {
    allowed: bool,
    message: String,
}

fn evaluate_remote_command_policy(
    request: &DockAgentRemoteCommandRequest,
) -> Result<RemoteCommandPolicyDecision, DockAgentError> {
    let policy = parse_json_value(&request.policy_json, "policy_json")?;
    let allowed_commands = string_array_field(&policy, "allowedCommands")?;
    if allowed_commands.is_empty() {
        return Ok(RemoteCommandPolicyDecision {
            allowed: false,
            message: "Policy must explicitly allow at least one command.".to_string(),
        });
    }
    if !allowed_commands
        .iter()
        .any(|command| command == request.command.trim())
    {
        return Ok(RemoteCommandPolicyDecision {
            allowed: false,
            message: format!(
                "Command `{}` is not allowed by policy.",
                request.command.trim()
            ),
        });
    }

    let denied_commands = string_array_field(&policy, "deniedCommands")?;
    if denied_commands
        .iter()
        .any(|command| command == request.command.trim())
    {
        return Ok(RemoteCommandPolicyDecision {
            allowed: false,
            message: format!("Command `{}` is explicitly denied.", request.command.trim()),
        });
    }

    let allowed_working_dirs = string_array_field(&policy, "allowedWorkingDirs")?;
    if !allowed_working_dirs.is_empty() {
        let working_dir = request.working_dir.as_deref().unwrap_or("");
        if working_dir.is_empty()
            || !allowed_working_dirs.iter().any(|allowed| {
                working_dir == allowed || working_dir.starts_with(&format!("{allowed}/"))
            })
        {
            return Ok(RemoteCommandPolicyDecision {
                allowed: false,
                message: "Working directory is not allowed by policy.".to_string(),
            });
        }
    }

    Ok(RemoteCommandPolicyDecision {
        allowed: true,
        message: "Remote command approved by policy.".to_string(),
    })
}

fn string_array_field(value: &Value, field_name: &str) -> Result<Vec<String>, DockAgentError> {
    match value.get(field_name) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(|raw| raw.to_string()).ok_or_else(|| {
                    DockAgentError::Validation(format!(
                        "policy field {field_name} must contain only strings"
                    ))
                })
            })
            .collect(),
        Some(_) => Err(DockAgentError::Validation(format!(
            "policy field {field_name} must be an array"
        ))),
    }
}

fn transition_remote_command(
    connection: &mut Connection,
    id: &str,
    next_status: DockAgentRemoteCommandStatus,
    actor: &str,
    details_json: &str,
    changed_at: &str,
    result_json: Option<&str>,
) -> Result<DockAgentRemoteCommand, DockAgentError> {
    if actor.trim().is_empty() {
        return Err(DockAgentError::Validation(
            "audit actor is required for remote command transitions".to_string(),
        ));
    }
    validate_json(details_json, "details_json")?;
    parse_timestamp(changed_at)?;

    let transaction = connection.transaction()?;
    let current = get_remote_command_from_connection(&transaction, id)?;
    validate_remote_command_transition(current.status, next_status)?;
    let status = enum_to_db_value(next_status)?;
    let executed_at = if next_status == DockAgentRemoteCommandStatus::Running {
        Some(changed_at)
    } else {
        current.executed_at.as_deref()
    };
    let result_json = result_json.unwrap_or(&current.result_json);

    transaction.execute(
        "UPDATE dock_agent_remote_commands
         SET status = ?1,
             executed_at = ?2,
             result_json = ?3,
             updated_at = ?4
         WHERE id = ?5",
        params![status, executed_at, result_json, changed_at, id],
    )?;
    let action = match next_status {
        DockAgentRemoteCommandStatus::Running => "remote_command.started",
        DockAgentRemoteCommandStatus::Succeeded => "remote_command.succeeded",
        DockAgentRemoteCommandStatus::Failed => "remote_command.failed",
        _ => "remote_command.transitioned",
    };
    let outcome = if next_status == DockAgentRemoteCommandStatus::Failed {
        DockAgentAuditOutcome::Failed
    } else {
        DockAgentAuditOutcome::Succeeded
    };
    insert_audit_log_in_connection(
        &transaction,
        actor,
        action,
        "dock_agent_remote_command",
        Some(id),
        outcome,
        details_json,
    )?;
    let updated = get_remote_command_from_connection(&transaction, id)?;
    transaction.commit()?;
    Ok(updated)
}

fn validate_remote_command_transition(
    current: DockAgentRemoteCommandStatus,
    next: DockAgentRemoteCommandStatus,
) -> Result<(), DockAgentError> {
    let allowed = matches!(
        (current, next),
        (
            DockAgentRemoteCommandStatus::Approved,
            DockAgentRemoteCommandStatus::Running
        ) | (
            DockAgentRemoteCommandStatus::Running,
            DockAgentRemoteCommandStatus::Succeeded
        ) | (
            DockAgentRemoteCommandStatus::Running,
            DockAgentRemoteCommandStatus::Failed
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(DockAgentError::InvalidRemoteCommandTransition {
            from: current,
            to: next,
        })
    }
}

fn chat_task_id(request: &DockAgentChatCommandRequest) -> String {
    let message_key = request
        .external_message_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&request.received_at);
    format!(
        "chat-{}-{}",
        sanitize_id_segment(&request.connector_id),
        sanitize_id_segment(message_key)
    )
}

fn sanitize_id_segment(raw: &str) -> String {
    let sanitized = raw
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn scheduled_task_id(schedule_id: &str, due_at: &str) -> String {
    let stable_due = due_at
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!("schedule-{schedule_id}-{stable_due}")
}

fn next_run_at_after(
    schedule: &DockAgentSchedule,
    due_at: &str,
) -> Result<Option<String>, DockAgentError> {
    let due_at = parse_timestamp(due_at)?;
    let next = match schedule.cadence {
        DockAgentScheduleCadence::Hourly => Some(due_at + Duration::hours(1)),
        DockAgentScheduleCadence::Weekly => Some(due_at + Duration::weeks(1)),
        DockAgentScheduleCadence::Cron => None,
    };

    Ok(next.map(format_timestamp))
}

fn parse_timestamp(raw: &str) -> Result<DateTime<Utc>, DockAgentError> {
    Ok(DateTime::parse_from_rfc3339(raw)?.with_timezone(&Utc))
}

fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn enum_to_db_value<T>(value: T) -> Result<String, DockAgentError>
where
    T: Serialize,
{
    match serde_json::to_value(value)? {
        Value::String(value) => Ok(value),
        _ => Err(DockAgentError::Validation(
            "enum value did not serialize to a string".to_string(),
        )),
    }
}

fn enum_from_db_value<T>(value: String) -> Result<T, rusqlite::Error>
where
    T: DeserializeOwned,
{
    serde_json::from_value(Value::String(value)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn provider_from_db_value(value: Option<String>) -> Result<Option<ProviderId>, rusqlite::Error> {
    value.map(enum_from_db_value).transpose()
}

fn map_task_row(row: &rusqlite::Row) -> Result<DockAgentTask, rusqlite::Error> {
    Ok(DockAgentTask {
        id: row.get(0)?,
        kind: enum_from_db_value::<DockAgentTaskKind>(row.get(1)?)?,
        status: enum_from_db_value::<DockAgentTaskStatus>(row.get(2)?)?,
        title: row.get(3)?,
        objective: row.get(4)?,
        provider_id: provider_from_db_value(row.get(5)?)?,
        target_thread_id: row.get(6)?,
        schedule_id: row.get(7)?,
        chat_connector_id: row.get(8)?,
        remote_command_id: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        started_at: row.get(12)?,
        completed_at: row.get(13)?,
        metadata_json: row.get(14)?,
    })
}

fn map_schedule_row(row: &rusqlite::Row) -> Result<DockAgentSchedule, rusqlite::Error> {
    Ok(DockAgentSchedule {
        id: row.get(0)?,
        task_id: row.get(1)?,
        cadence: enum_from_db_value::<DockAgentScheduleCadence>(row.get(2)?)?,
        cron_expression: row.get(3)?,
        timezone: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        next_run_at: row.get(6)?,
        last_run_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn map_chat_connector_row(row: &rusqlite::Row) -> Result<DockAgentChatConnector, rusqlite::Error> {
    Ok(DockAgentChatConnector {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: enum_from_db_value::<DockAgentChatConnectorKind>(row.get(2)?)?,
        enabled: row.get::<_, i64>(3)? != 0,
        config_json: row.get(4)?,
        secret_ref: row.get(5)?,
        last_seen_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_remote_command_row(row: &rusqlite::Row) -> Result<DockAgentRemoteCommand, rusqlite::Error> {
    Ok(DockAgentRemoteCommand {
        id: row.get(0)?,
        task_id: row.get(1)?,
        device_id: row.get(2)?,
        status: enum_from_db_value::<DockAgentRemoteCommandStatus>(row.get(3)?)?,
        command: row.get(4)?,
        args_json: row.get(5)?,
        working_dir: row.get(6)?,
        policy_json: row.get(7)?,
        requested_by: row.get(8)?,
        approved_at: row.get(9)?,
        executed_at: row.get(10)?,
        result_json: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn map_audit_log_row(row: &rusqlite::Row) -> Result<DockAgentAuditLog, rusqlite::Error> {
    Ok(DockAgentAuditLog {
        id: row.get(0)?,
        actor: row.get(1)?,
        action: row.get(2)?,
        subject_type: row.get(3)?,
        subject_id: row.get(4)?,
        outcome: enum_from_db_value::<DockAgentAuditOutcome>(row.get(5)?)?,
        details_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        complete_remote_command, create_chat_connector, create_schedule, create_task,
        enqueue_due_schedules, get_chat_connector, get_remote_command, get_schedule, get_task,
        handle_chat_command, list_audit_logs, list_due_schedules, list_tasks_by_status,
        request_remote_command, start_remote_command, transition_task, DockAgentError,
    };
    use crate::db::run_migrations;
    use provider_contract::{
        DockAgentChatCommandRequest, DockAgentChatConnector, DockAgentChatConnectorKind,
        DockAgentRemoteCommandRequest, DockAgentRemoteCommandStatus, DockAgentSchedule,
        DockAgentScheduleCadence, DockAgentTask, DockAgentTaskKind, DockAgentTaskStatus,
        ProviderId,
    };
    use rusqlite::Connection;

    fn setup_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("in-memory db should open");
        run_migrations(&mut connection).expect("migrations should run");
        connection
            .execute(
                "INSERT INTO providers(id, name, status) VALUES
                 ('codex', 'Codex', 'healthy'),
                 ('claude_code', 'Claude Code', 'healthy'),
                 ('opencode', 'OpenCode', 'healthy')",
                [],
            )
            .expect("providers should insert");
        connection
            .execute(
                "INSERT INTO remote_devices(id, device_name, paired_at) VALUES
                 ('device-1', 'Demo Device', '2026-06-03T00:00:00.000Z')",
                [],
            )
            .expect("remote device should insert");
        connection
    }

    fn sample_task(id: &str) -> DockAgentTask {
        DockAgentTask {
            id: id.to_string(),
            kind: DockAgentTaskKind::Orchestration,
            status: DockAgentTaskStatus::Queued,
            title: "Review stale work".to_string(),
            objective: "Inspect open worker threads and resume the highest-priority one."
                .to_string(),
            provider_id: Some(ProviderId::Codex),
            target_thread_id: None,
            schedule_id: None,
            chat_connector_id: None,
            remote_command_id: None,
            created_at: "2026-06-03T00:00:00.000Z".to_string(),
            updated_at: "2026-06-03T00:00:00.000Z".to_string(),
            started_at: None,
            completed_at: None,
            metadata_json: "{\"priority\":\"high\"}".to_string(),
        }
    }

    fn sample_schedule(id: &str, cadence: DockAgentScheduleCadence) -> DockAgentSchedule {
        DockAgentSchedule {
            id: id.to_string(),
            task_id: None,
            cadence,
            cron_expression: None,
            timezone: "UTC".to_string(),
            enabled: true,
            next_run_at: Some("2026-06-03T01:00:00.000Z".to_string()),
            last_run_at: None,
            created_at: "2026-06-03T00:00:00.000Z".to_string(),
            updated_at: "2026-06-03T00:00:00.000Z".to_string(),
        }
    }

    fn sample_chat_connector(id: &str) -> DockAgentChatConnector {
        DockAgentChatConnector {
            id: id.to_string(),
            name: "Team Chat".to_string(),
            kind: DockAgentChatConnectorKind::Webhook,
            enabled: true,
            config_json: "{\"channel\":\"agentdock\"}".to_string(),
            secret_ref: Some("keychain://dock-agent/team-chat".to_string()),
            last_seen_at: None,
            created_at: "2026-06-03T00:00:00.000Z".to_string(),
            updated_at: "2026-06-03T00:00:00.000Z".to_string(),
        }
    }

    fn sample_chat_request(connector_id: &str) -> DockAgentChatCommandRequest {
        DockAgentChatCommandRequest {
            connector_id: connector_id.to_string(),
            external_message_id: Some("msg-1".to_string()),
            actor: "alice".to_string(),
            text: "Check provider health and queue the next repair.".to_string(),
            received_at: "2026-06-03T01:00:00.000Z".to_string(),
            raw_payload_json: "{\"text\":\"Check provider health and queue the next repair.\"}"
                .to_string(),
        }
    }

    fn sample_remote_command_request(id: &str) -> DockAgentRemoteCommandRequest {
        DockAgentRemoteCommandRequest {
            id: id.to_string(),
            task_id: None,
            device_id: Some("device-1".to_string()),
            command: "git".to_string(),
            args_json: "[\"status\",\"--short\"]".to_string(),
            working_dir: Some("/workspace/demo".to_string()),
            policy_json: "{\"allowedCommands\":[\"git\"],\"allowedWorkingDirs\":[\"/workspace\"]}"
                .to_string(),
            requested_by: "alice".to_string(),
            requested_at: "2026-06-03T01:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn create_and_get_task_round_trips_core_fields() {
        let connection = setup_connection();
        let task = sample_task("task-1");

        create_task(&connection, &task).expect("task should create");
        let loaded = get_task(&connection, "task-1").expect("task should load");

        assert_eq!(loaded.id, "task-1");
        assert_eq!(loaded.kind, DockAgentTaskKind::Orchestration);
        assert_eq!(loaded.status, DockAgentTaskStatus::Queued);
        assert_eq!(loaded.provider_id, Some(ProviderId::Codex));
        assert_eq!(loaded.metadata_json, "{\"priority\":\"high\"}");
    }

    #[test]
    fn list_tasks_by_status_filters_queue() {
        let connection = setup_connection();
        let mut running = sample_task("task-running");
        running.status = DockAgentTaskStatus::Running;
        let queued = sample_task("task-queued");

        create_task(&connection, &running).expect("running task should create");
        create_task(&connection, &queued).expect("queued task should create");

        let queued_tasks = list_tasks_by_status(&connection, Some(DockAgentTaskStatus::Queued))
            .expect("queued tasks should load");

        assert_eq!(queued_tasks.len(), 1);
        assert_eq!(queued_tasks[0].id, "task-queued");
    }

    #[test]
    fn transition_task_updates_status_timestamps_and_audit_log() {
        let mut connection = setup_connection();
        create_task(&connection, &sample_task("task-1")).expect("task should create");

        let running = transition_task(
            &mut connection,
            "task-1",
            DockAgentTaskStatus::Running,
            "dock_agent",
            "{\"reason\":\"worker claimed\"}",
            "2026-06-03T01:00:00.000Z",
        )
        .expect("queued task should transition to running");
        let succeeded = transition_task(
            &mut connection,
            "task-1",
            DockAgentTaskStatus::Succeeded,
            "dock_agent",
            "{\"reason\":\"worker finished\"}",
            "2026-06-03T01:05:00.000Z",
        )
        .expect("running task should transition to succeeded");

        assert_eq!(running.status, DockAgentTaskStatus::Running);
        assert_eq!(
            running.started_at.as_deref(),
            Some("2026-06-03T01:00:00.000Z")
        );
        assert_eq!(succeeded.status, DockAgentTaskStatus::Succeeded);
        assert_eq!(
            succeeded.started_at.as_deref(),
            Some("2026-06-03T01:00:00.000Z")
        );
        assert_eq!(
            succeeded.completed_at.as_deref(),
            Some("2026-06-03T01:05:00.000Z")
        );

        let logs = list_audit_logs(&connection, Some("dock_agent_task"), Some("task-1"))
            .expect("audit logs should load");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].action, "task.transition");
        assert_eq!(logs[0].actor, "dock_agent");
    }

    #[test]
    fn transition_task_rejects_invalid_state_change_without_audit_log() {
        let mut connection = setup_connection();
        create_task(&connection, &sample_task("task-1")).expect("task should create");

        let error = transition_task(
            &mut connection,
            "task-1",
            DockAgentTaskStatus::Succeeded,
            "dock_agent",
            "{}",
            "2026-06-03T01:00:00.000Z",
        )
        .expect_err("queued task cannot jump to succeeded");

        assert!(matches!(error, DockAgentError::InvalidTransition { .. }));
        let loaded = get_task(&connection, "task-1").expect("task should still load");
        assert_eq!(loaded.status, DockAgentTaskStatus::Queued);
        let logs = list_audit_logs(&connection, Some("dock_agent_task"), Some("task-1"))
            .expect("audit logs should load");
        assert!(logs.is_empty());
    }

    #[test]
    fn create_and_get_schedule_round_trips_hourly_fields() {
        let connection = setup_connection();
        let schedule = sample_schedule("schedule-1", DockAgentScheduleCadence::Hourly);

        create_schedule(&connection, &schedule).expect("schedule should create");
        let loaded = get_schedule(&connection, "schedule-1").expect("schedule should load");

        assert_eq!(loaded.id, "schedule-1");
        assert_eq!(loaded.cadence, DockAgentScheduleCadence::Hourly);
        assert!(loaded.enabled);
        assert_eq!(
            loaded.next_run_at.as_deref(),
            Some("2026-06-03T01:00:00.000Z")
        );
    }

    #[test]
    fn list_due_schedules_excludes_disabled_and_future_schedules() {
        let connection = setup_connection();
        let due = sample_schedule("schedule-due", DockAgentScheduleCadence::Hourly);
        let mut future = sample_schedule("schedule-future", DockAgentScheduleCadence::Hourly);
        future.next_run_at = Some("2026-06-03T03:00:00.000Z".to_string());
        let mut disabled = sample_schedule("schedule-disabled", DockAgentScheduleCadence::Hourly);
        disabled.enabled = false;

        create_schedule(&connection, &due).expect("due schedule should create");
        create_schedule(&connection, &future).expect("future schedule should create");
        create_schedule(&connection, &disabled).expect("disabled schedule should create");

        let schedules = list_due_schedules(&connection, "2026-06-03T02:00:00.000Z")
            .expect("due schedules should load");

        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].id, "schedule-due");
    }

    #[test]
    fn enqueue_due_hourly_schedule_creates_scheduled_task_and_advances_next_run() {
        let mut connection = setup_connection();
        create_schedule(
            &connection,
            &sample_schedule("schedule-1", DockAgentScheduleCadence::Hourly),
        )
        .expect("schedule should create");

        let enqueued =
            enqueue_due_schedules(&mut connection, "2026-06-03T02:00:00.000Z", "scheduler")
                .expect("due schedule should enqueue");

        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0].task.kind, DockAgentTaskKind::ScheduledCheck);
        assert_eq!(enqueued[0].task.status, DockAgentTaskStatus::Queued);
        assert_eq!(enqueued[0].task.schedule_id.as_deref(), Some("schedule-1"));
        let loaded_schedule =
            get_schedule(&connection, "schedule-1").expect("schedule should load");
        assert_eq!(
            loaded_schedule.last_run_at.as_deref(),
            Some("2026-06-03T01:00:00.000Z")
        );
        assert_eq!(
            loaded_schedule.next_run_at.as_deref(),
            Some("2026-06-03T02:00:00.000Z")
        );
        let logs = list_audit_logs(&connection, Some("dock_agent_schedule"), Some("schedule-1"))
            .expect("schedule audit logs should load");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "schedule.enqueue_due");
    }

    #[test]
    fn enqueue_due_weekly_schedule_advances_by_one_week() {
        let mut connection = setup_connection();
        create_schedule(
            &connection,
            &sample_schedule("schedule-weekly", DockAgentScheduleCadence::Weekly),
        )
        .expect("schedule should create");

        enqueue_due_schedules(&mut connection, "2026-06-03T02:00:00.000Z", "scheduler")
            .expect("due schedule should enqueue");

        let loaded_schedule =
            get_schedule(&connection, "schedule-weekly").expect("schedule should load");
        assert_eq!(
            loaded_schedule.next_run_at.as_deref(),
            Some("2026-06-10T01:00:00.000Z")
        );
    }

    #[test]
    fn enqueue_due_schedule_uses_template_task_fields() {
        let mut connection = setup_connection();
        let template = sample_task("template-task");
        create_task(&connection, &template).expect("template should create");
        let mut schedule = sample_schedule("schedule-template", DockAgentScheduleCadence::Hourly);
        schedule.task_id = Some("template-task".to_string());
        create_schedule(&connection, &schedule).expect("schedule should create");

        let enqueued =
            enqueue_due_schedules(&mut connection, "2026-06-03T02:00:00.000Z", "scheduler")
                .expect("due schedule should enqueue");

        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0].task.title, template.title);
        assert_eq!(enqueued[0].task.objective, template.objective);
        assert_eq!(enqueued[0].task.provider_id, Some(ProviderId::Codex));
    }

    #[test]
    fn enqueue_due_schedules_is_idempotent_for_same_due_time() {
        let mut connection = setup_connection();
        create_schedule(
            &connection,
            &sample_schedule("schedule-1", DockAgentScheduleCadence::Hourly),
        )
        .expect("schedule should create");

        let first = enqueue_due_schedules(&mut connection, "2026-06-03T01:30:00.000Z", "scheduler")
            .expect("first scan should enqueue");
        let second =
            enqueue_due_schedules(&mut connection, "2026-06-03T01:30:00.000Z", "scheduler")
                .expect("second scan should not enqueue same due time");

        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
        let queued_tasks = list_tasks_by_status(&connection, Some(DockAgentTaskStatus::Queued))
            .expect("queued tasks should load");
        assert_eq!(queued_tasks.len(), 1);
    }

    #[test]
    fn create_schedule_requires_cron_expression_for_cron_cadence() {
        let connection = setup_connection();
        let schedule = sample_schedule("schedule-cron", DockAgentScheduleCadence::Cron);

        let error =
            create_schedule(&connection, &schedule).expect_err("cron expression is required");

        assert!(matches!(error, DockAgentError::Validation(_)));
    }

    #[test]
    fn create_and_get_chat_connector_round_trips_without_secret_material() {
        let connection = setup_connection();
        let connector = sample_chat_connector("chat-1");

        create_chat_connector(&connection, &connector).expect("connector should create");
        let loaded = get_chat_connector(&connection, "chat-1").expect("connector should load");

        assert_eq!(loaded.id, "chat-1");
        assert_eq!(loaded.kind, DockAgentChatConnectorKind::Webhook);
        assert_eq!(
            loaded.secret_ref.as_deref(),
            Some("keychain://dock-agent/team-chat")
        );
        assert_eq!(loaded.config_json, "{\"channel\":\"agentdock\"}");
    }

    #[test]
    fn create_chat_connector_rejects_secret_like_config_keys() {
        let connection = setup_connection();
        let mut connector = sample_chat_connector("chat-1");
        connector.config_json = "{\"signingSecret\":\"inline-secret\"}".to_string();

        let error = create_chat_connector(&connection, &connector)
            .expect_err("inline secrets should reject");

        assert!(matches!(error, DockAgentError::Validation(_)));
    }

    #[test]
    fn handle_chat_command_accepts_webhook_and_queues_task() {
        let mut connection = setup_connection();
        create_chat_connector(&connection, &sample_chat_connector("chat-1"))
            .expect("connector should create");

        let result = handle_chat_command(&mut connection, &sample_chat_request("chat-1"))
            .expect("chat command should handle");

        assert!(result.accepted);
        let task = result.task.expect("accepted command should create task");
        assert_eq!(task.kind, DockAgentTaskKind::ChatCommand);
        assert_eq!(task.status, DockAgentTaskStatus::Queued);
        assert_eq!(task.chat_connector_id.as_deref(), Some("chat-1"));
        assert_eq!(
            task.objective,
            "Check provider health and queue the next repair."
        );
        let loaded_connector =
            get_chat_connector(&connection, "chat-1").expect("connector should load");
        assert_eq!(
            loaded_connector.last_seen_at.as_deref(),
            Some("2026-06-03T01:00:00.000Z")
        );
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_chat_connector"),
            Some("chat-1"),
        )
        .expect("chat audit logs should load");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "chat.command_accepted");
        assert_eq!(
            logs[0].outcome,
            provider_contract::DockAgentAuditOutcome::Allowed
        );
    }

    #[test]
    fn handle_chat_command_is_idempotent_for_external_message_id() {
        let mut connection = setup_connection();
        create_chat_connector(&connection, &sample_chat_connector("chat-1"))
            .expect("connector should create");
        let request = sample_chat_request("chat-1");

        let first =
            handle_chat_command(&mut connection, &request).expect("first command should handle");
        let second =
            handle_chat_command(&mut connection, &request).expect("second command should handle");

        assert!(first.accepted);
        assert!(first.task.is_some());
        assert!(second.accepted);
        assert!(second.task.is_none());
        let queued_tasks = list_tasks_by_status(&connection, Some(DockAgentTaskStatus::Queued))
            .expect("queued tasks should load");
        assert_eq!(queued_tasks.len(), 1);
    }

    #[test]
    fn handle_chat_command_rejects_disabled_connector_with_audit_log() {
        let mut connection = setup_connection();
        let mut connector = sample_chat_connector("chat-1");
        connector.enabled = false;
        create_chat_connector(&connection, &connector).expect("connector should create");

        let result = handle_chat_command(&mut connection, &sample_chat_request("chat-1"))
            .expect("disabled connector should return rejected result");

        assert!(!result.accepted);
        assert!(result.task.is_none());
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_chat_connector"),
            Some("chat-1"),
        )
        .expect("chat audit logs should load");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "chat.command_rejected");
        assert_eq!(
            logs[0].outcome,
            provider_contract::DockAgentAuditOutcome::Denied
        );
    }

    #[test]
    fn handle_chat_command_rejects_unknown_connector_with_audit_log() {
        let mut connection = setup_connection();

        let result = handle_chat_command(&mut connection, &sample_chat_request("missing-chat"))
            .expect("unknown connector should return rejected result");

        assert!(!result.accepted);
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_chat_connector"),
            Some("missing-chat"),
        )
        .expect("chat audit logs should load");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "chat.command_rejected");
    }

    #[test]
    fn request_remote_command_approves_allowed_command_and_queues_task() {
        let mut connection = setup_connection();
        let request = sample_remote_command_request("remote-1");

        let decision = request_remote_command(&mut connection, &request)
            .expect("remote command request should evaluate");

        assert!(decision.accepted);
        assert_eq!(
            decision.command.status,
            DockAgentRemoteCommandStatus::Approved
        );
        assert_eq!(
            decision.command.approved_at.as_deref(),
            Some(request.requested_at.as_str())
        );
        assert!(decision.command.task_id.is_some());
        let queued_tasks = list_tasks_by_status(&connection, Some(DockAgentTaskStatus::Queued))
            .expect("queued tasks should load");
        assert_eq!(queued_tasks.len(), 1);
        assert_eq!(queued_tasks[0].kind, DockAgentTaskKind::RemoteCommand);
        assert_eq!(
            queued_tasks[0].remote_command_id.as_deref(),
            Some("remote-1")
        );
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_remote_command"),
            Some("remote-1"),
        )
        .expect("remote command audit logs should load");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "remote_command.approved");
    }

    #[test]
    fn request_remote_command_rejects_command_not_in_allowlist() {
        let mut connection = setup_connection();
        let mut request = sample_remote_command_request("remote-1");
        request.command = "rm".to_string();

        let decision = request_remote_command(&mut connection, &request)
            .expect("remote command request should evaluate");

        assert!(!decision.accepted);
        assert_eq!(
            decision.command.status,
            DockAgentRemoteCommandStatus::Rejected
        );
        assert!(decision.command.task_id.is_none());
        let queued_tasks = list_tasks_by_status(&connection, Some(DockAgentTaskStatus::Queued))
            .expect("queued tasks should load");
        assert!(queued_tasks.is_empty());
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_remote_command"),
            Some("remote-1"),
        )
        .expect("remote command audit logs should load");
        assert_eq!(logs[0].action, "remote_command.rejected");
        assert_eq!(
            logs[0].outcome,
            provider_contract::DockAgentAuditOutcome::Denied
        );
    }

    #[test]
    fn request_remote_command_rejects_disallowed_working_dir() {
        let mut connection = setup_connection();
        let mut request = sample_remote_command_request("remote-1");
        request.working_dir = Some("/tmp/demo".to_string());

        let decision = request_remote_command(&mut connection, &request)
            .expect("remote command request should evaluate");

        assert!(!decision.accepted);
        assert_eq!(
            decision.command.status,
            DockAgentRemoteCommandStatus::Rejected
        );
        assert!(decision
            .message
            .as_deref()
            .is_some_and(|message| message.contains("Working directory")));
    }

    #[test]
    fn remote_command_runs_and_completes_after_approval() {
        let mut connection = setup_connection();
        request_remote_command(&mut connection, &sample_remote_command_request("remote-1"))
            .expect("remote command request should approve");

        let running = start_remote_command(
            &mut connection,
            "remote-1",
            "dock_agent",
            "2026-06-03T01:01:00.000Z",
        )
        .expect("approved command should start");
        let completed = complete_remote_command(
            &mut connection,
            "remote-1",
            true,
            "dock_agent",
            "{\"exitCode\":0,\"stdout\":\"clean\"}",
            "2026-06-03T01:02:00.000Z",
        )
        .expect("running command should complete");

        assert_eq!(running.status, DockAgentRemoteCommandStatus::Running);
        assert_eq!(completed.status, DockAgentRemoteCommandStatus::Succeeded);
        assert_eq!(
            completed.executed_at.as_deref(),
            Some("2026-06-03T01:01:00.000Z")
        );
        assert_eq!(
            completed.result_json,
            "{\"exitCode\":0,\"stdout\":\"clean\"}"
        );
        let logs = list_audit_logs(
            &connection,
            Some("dock_agent_remote_command"),
            Some("remote-1"),
        )
        .expect("remote command audit logs should load");
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].action, "remote_command.succeeded");
    }

    #[test]
    fn rejected_remote_command_cannot_start() {
        let mut connection = setup_connection();
        let mut request = sample_remote_command_request("remote-1");
        request.command = "rm".to_string();
        request_remote_command(&mut connection, &request)
            .expect("remote command request should reject");

        let error = start_remote_command(
            &mut connection,
            "remote-1",
            "dock_agent",
            "2026-06-03T01:01:00.000Z",
        )
        .expect_err("rejected command must not start");

        assert!(matches!(
            error,
            DockAgentError::InvalidRemoteCommandTransition { .. }
        ));
        let command =
            get_remote_command(&connection, "remote-1").expect("remote command should load");
        assert_eq!(command.status, DockAgentRemoteCommandStatus::Rejected);
    }
}
