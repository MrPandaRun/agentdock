import type { ProviderId } from "./provider";

export type DockAgentTaskKind =
  | "orchestration"
  | "scheduled_check"
  | "chat_command"
  | "remote_command";

export type DockAgentTaskStatus =
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "canceled";

export type DockAgentScheduleCadence = "hourly" | "weekly" | "cron";

export type DockAgentChatConnectorKind = "webhook";

export type DockAgentRemoteCommandStatus =
  | "requested"
  | "approved"
  | "running"
  | "succeeded"
  | "failed"
  | "rejected";

export type DockAgentAuditOutcome = "allowed" | "denied" | "succeeded" | "failed";

export interface DockAgentTask {
  id: string;
  kind: DockAgentTaskKind;
  status: DockAgentTaskStatus;
  title: string;
  objective: string;
  providerId?: ProviderId;
  targetThreadId?: string;
  scheduleId?: string;
  chatConnectorId?: string;
  remoteCommandId?: string;
  createdAt: string;
  updatedAt: string;
  startedAt?: string;
  completedAt?: string;
  metadataJson: string;
}

export interface DockAgentSchedule {
  id: string;
  taskId?: string;
  cadence: DockAgentScheduleCadence;
  cronExpression?: string;
  timezone: string;
  enabled: boolean;
  nextRunAt?: string;
  lastRunAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface DockAgentChatConnector {
  id: string;
  name: string;
  kind: DockAgentChatConnectorKind;
  enabled: boolean;
  configJson: string;
  secretRef?: string;
  lastSeenAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface DockAgentChatCommandRequest {
  connectorId: string;
  externalMessageId?: string;
  actor: string;
  text: string;
  receivedAt: string;
  rawPayloadJson: string;
}

export interface DockAgentChatCommandResult {
  accepted: boolean;
  task?: DockAgentTask;
  message?: string;
}

export interface DockAgentRemoteCommand {
  id: string;
  taskId?: string;
  deviceId?: string;
  status: DockAgentRemoteCommandStatus;
  command: string;
  argsJson: string;
  workingDir?: string;
  policyJson: string;
  requestedBy: string;
  approvedAt?: string;
  executedAt?: string;
  resultJson: string;
  createdAt: string;
  updatedAt: string;
}

export interface DockAgentRemoteCommandRequest {
  id: string;
  taskId?: string;
  deviceId?: string;
  command: string;
  argsJson: string;
  workingDir?: string;
  policyJson: string;
  requestedBy: string;
  requestedAt: string;
}

export interface DockAgentRemoteCommandDecision {
  accepted: boolean;
  command: DockAgentRemoteCommand;
  message?: string;
}

export interface DockAgentAuditLog {
  id: number;
  actor: string;
  action: string;
  subjectType: string;
  subjectId?: string;
  outcome: DockAgentAuditOutcome;
  detailsJson: string;
  createdAt: string;
}
