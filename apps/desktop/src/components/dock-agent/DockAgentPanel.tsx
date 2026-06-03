import { invoke } from "@tauri-apps/api/core";
import {
  Bot,
  CalendarClock,
  CheckCircle2,
  ClipboardList,
  History,
  Loader2,
  Play,
  RefreshCw,
  ShieldCheck,
  TerminalSquare,
  Webhook,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactElement } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { JsonCodeEditor } from "@/components/ui/json-code-editor";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { providerDisplayName } from "@/lib/provider";
import { cn } from "@/lib/utils";
import type { AgentThreadSummary } from "@/types";

type DockAgentProviderId = "codex" | "claude_code" | "opencode";
type DockAgentTaskStatus = "queued" | "running" | "succeeded" | "failed" | "canceled";
type DockAgentTaskKind = "orchestration" | "scheduled_check" | "chat_command" | "remote_command";
type DockAgentScheduleCadence = "hourly" | "weekly" | "cron";

interface DockAgentTask {
  id: string;
  kind: DockAgentTaskKind;
  status: DockAgentTaskStatus;
  title: string;
  objective: string;
  providerId?: DockAgentProviderId | null;
  targetThreadId?: string | null;
  scheduleId?: string | null;
  chatConnectorId?: string | null;
  remoteCommandId?: string | null;
  createdAt: string;
  updatedAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
  metadataJson: string;
}

interface DockAgentSchedule {
  id: string;
  taskId?: string | null;
  cadence: DockAgentScheduleCadence;
  cronExpression?: string | null;
  timezone: string;
  enabled: boolean;
  nextRunAt?: string | null;
  lastRunAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

interface DockAgentChatConnector {
  id: string;
  name: string;
  kind: "webhook";
  enabled: boolean;
  configJson: string;
  secretRef?: string | null;
  lastSeenAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

interface DockAgentRemoteCommand {
  id: string;
  taskId?: string | null;
  deviceId?: string | null;
  status: "requested" | "approved" | "running" | "succeeded" | "failed" | "rejected";
  command: string;
  argsJson: string;
  workingDir?: string | null;
  policyJson: string;
  requestedBy: string;
  approvedAt?: string | null;
  executedAt?: string | null;
  resultJson: string;
  createdAt: string;
  updatedAt: string;
}

interface DockAgentRemoteCommandDecision {
  accepted: boolean;
  command: DockAgentRemoteCommand;
  message?: string | null;
}

interface DockAgentChatCommandResult {
  accepted: boolean;
  task?: DockAgentTask | null;
  message?: string | null;
}

interface DockAgentAuditLog {
  id: number;
  actor: string;
  action: string;
  subjectType: string;
  subjectId?: string | null;
  outcome: "allowed" | "denied" | "succeeded" | "failed" | string;
  detailsJson: string;
  createdAt: string;
}

interface DockAgentPanelProps {
  selectedThread: AgentThreadSummary | null;
  darkMode: boolean;
}

const PROVIDERS: DockAgentProviderId[] = ["codex", "claude_code", "opencode"];
const STATUS_FILTERS: Array<DockAgentTaskStatus | "all"> = [
  "all",
  "queued",
  "running",
  "succeeded",
  "failed",
  "canceled",
];

function nowIso(): string {
  return new Date().toISOString();
}

function uniqueId(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function formatShortTime(value?: string | null): string {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function compactJson(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value));
  } catch {
    return value;
  }
}

function providerFromThread(thread: AgentThreadSummary | null): DockAgentProviderId {
  if (thread?.providerId === "claude_code" || thread?.providerId === "opencode") {
    return thread.providerId;
  }
  return "codex";
}

function providerOptions(): ReactElement[] {
  return PROVIDERS.map((providerId) => (
    <option key={providerId} value={providerId}>
      {providerDisplayName(providerId)}
    </option>
  ));
}

function statusTone(status: string): string {
  if (status === "queued") {
    return "border-amber-500/35 bg-amber-500/10 text-amber-700 dark:text-amber-300";
  }
  if (status === "running" || status === "approved") {
    return "border-sky-500/35 bg-sky-500/10 text-sky-700 dark:text-sky-300";
  }
  if (status === "succeeded" || status === "allowed") {
    return "border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
  }
  if (status === "failed" || status === "denied" || status === "rejected") {
    return "border-red-500/35 bg-red-500/10 text-red-700 dark:text-red-300";
  }
  return "border-border bg-muted text-muted-foreground";
}

export default function DockAgentPanel({
  selectedThread,
  darkMode,
}: DockAgentPanelProps): ReactElement {
  const [taskFilter, setTaskFilter] = useState<(typeof STATUS_FILTERS)[number]>("all");
  const [tasks, setTasks] = useState<DockAgentTask[]>([]);
  const [dueSchedules, setDueSchedules] = useState<DockAgentSchedule[]>([]);
  const [auditLogs, setAuditLogs] = useState<DockAgentAuditLog[]>([]);
  const [loading, setLoading] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [taskTitle, setTaskTitle] = useState("Review active agent work");
  const [taskObjective, setTaskObjective] = useState(
    "Inspect recent provider threads, identify blocked work, and resume the highest-priority thread.",
  );
  const [taskProviderId, setTaskProviderId] = useState<DockAgentProviderId>(
    providerFromThread(selectedThread),
  );
  const [targetThreadId, setTargetThreadId] = useState(selectedThread?.id ?? "");
  const [taskMetadataJson, setTaskMetadataJson] = useState('{"source":"desktop"}');

  const [scheduleEnabled, setScheduleEnabled] = useState(true);
  const [scheduleCadence, setScheduleCadence] = useState<DockAgentScheduleCadence>("hourly");
  const [scheduleCron, setScheduleCron] = useState("0 * * * *");

  const [connectorName, setConnectorName] = useState("Team webhook");
  const [chatText, setChatText] = useState("dock agent: check stale work");

  const [remoteCommand, setRemoteCommand] = useState("git");
  const [remoteArgsJson, setRemoteArgsJson] = useState('["status","--short"]');
  const [remoteWorkingDir, setRemoteWorkingDir] = useState(selectedThread?.projectPath ?? "");
  const [remotePolicyJson, setRemotePolicyJson] = useState(
    '{"allowedCommands":["git"],"allowedWorkingDirs":[]}',
  );

  useEffect(() => {
    setTaskProviderId(providerFromThread(selectedThread));
    setTargetThreadId(selectedThread?.id ?? "");
    setRemoteWorkingDir(selectedThread?.projectPath ?? "");
  }, [selectedThread]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextTasks, nextDueSchedules, nextAuditLogs] = await Promise.all([
        invoke<DockAgentTask[]>("list_dock_agent_tasks", {
          request: { status: taskFilter === "all" ? null : taskFilter },
        }),
        invoke<DockAgentSchedule[]>("list_due_dock_agent_schedules", {
          now: nowIso(),
        }),
        invoke<DockAgentAuditLog[]>("list_dock_agent_audit_logs", {
          request: { subjectType: null, subjectId: null },
        }),
      ]);
      setTasks(nextTasks);
      setDueSchedules(nextDueSchedules);
      setAuditLogs(nextAuditLogs.slice(0, 30));
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setLoading(false);
    }
  }, [taskFilter]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const taskStats = useMemo(() => {
    return tasks.reduce<Record<DockAgentTaskStatus, number>>(
      (acc, task) => {
        acc[task.status] += 1;
        return acc;
      },
      { queued: 0, running: 0, succeeded: 0, failed: 0, canceled: 0 },
    );
  }, [tasks]);

  const selectedThreadIsMvpProvider = PROVIDERS.some(
    (providerId) => providerId === selectedThread?.providerId,
  );

  async function runAction(label: string, action: () => Promise<string>): Promise<void> {
    setActionLoading(label);
    setError(null);
    setMessage(null);
    try {
      const nextMessage = await action();
      setMessage(nextMessage);
      await refresh();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setActionLoading(null);
    }
  }

  async function createTask(): Promise<string> {
    const timestamp = nowIso();
    const task: DockAgentTask = {
      id: uniqueId("task"),
      kind: "orchestration",
      status: "queued",
      title: taskTitle.trim() || "Untitled Dock Agent task",
      objective: taskObjective.trim(),
      providerId: taskProviderId,
      targetThreadId: targetThreadId.trim() || null,
      scheduleId: null,
      chatConnectorId: null,
      remoteCommandId: null,
      createdAt: timestamp,
      updatedAt: timestamp,
      startedAt: null,
      completedAt: null,
      metadataJson: compactJson(taskMetadataJson),
    };
    const created = await invoke<DockAgentTask>("create_dock_agent_task", { request: task });
    return `Queued task ${created.id}.`;
  }

  async function createSchedule(): Promise<string> {
    const timestamp = nowIso();
    const taskId = uniqueId("task");
    const scheduleId = uniqueId("schedule");
    const task: DockAgentTask = {
      id: taskId,
      kind: "scheduled_check",
      status: "queued",
      title: `Scheduled ${taskTitle.trim() || "Dock Agent check"}`,
      objective: taskObjective.trim(),
      providerId: taskProviderId,
      targetThreadId: targetThreadId.trim() || null,
      scheduleId,
      chatConnectorId: null,
      remoteCommandId: null,
      createdAt: timestamp,
      updatedAt: timestamp,
      startedAt: null,
      completedAt: null,
      metadataJson: compactJson(taskMetadataJson),
    };
    await invoke<DockAgentTask>("create_dock_agent_task", { request: task });
    const schedule: DockAgentSchedule = {
      id: scheduleId,
      taskId,
      cadence: scheduleCadence,
      cronExpression: scheduleCadence === "cron" ? scheduleCron.trim() : null,
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
      enabled: scheduleEnabled,
      nextRunAt: timestamp,
      lastRunAt: null,
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    const created = await invoke<DockAgentSchedule>("create_dock_agent_schedule", {
      request: schedule,
    });
    return `Created ${created.cadence} schedule ${created.id}.`;
  }

  async function enqueueDueSchedules(): Promise<string> {
    const enqueued = await invoke<Array<{ schedule: DockAgentSchedule; task: DockAgentTask }>>(
      "enqueue_due_dock_agent_schedules",
      { request: { now: nowIso(), actor: "desktop" } },
    );
    return enqueued.length === 1
      ? "Enqueued 1 due schedule."
      : `Enqueued ${enqueued.length} due schedules.`;
  }

  async function sendChatCommand(): Promise<string> {
    const timestamp = nowIso();
    const connector: DockAgentChatConnector = {
      id: uniqueId("connector"),
      name: connectorName.trim() || "Desktop webhook",
      kind: "webhook",
      enabled: true,
      configJson: '{"source":"desktop"}',
      secretRef: "desktop://dock-agent/demo-webhook",
      lastSeenAt: null,
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    const created = await invoke<DockAgentChatConnector>("create_dock_agent_chat_connector", {
      request: connector,
    });
    const result = await invoke<DockAgentChatCommandResult>("handle_dock_agent_chat_command", {
      request: {
        connectorId: created.id,
        externalMessageId: uniqueId("message"),
        actor: "desktop",
        text: chatText.trim(),
        receivedAt: timestamp,
        rawPayloadJson: JSON.stringify({ text: chatText.trim(), source: "desktop" }),
      },
    });
    return result.accepted
      ? `Accepted chat command as task ${result.task?.id ?? "-"}.`
      : result.message ?? "Chat command was rejected.";
  }

  async function requestRemoteCommand(): Promise<string> {
    const timestamp = nowIso();
    const decision = await invoke<DockAgentRemoteCommandDecision>(
      "request_dock_agent_remote_command",
      {
        request: {
          id: uniqueId("remote"),
          taskId: null,
          deviceId: "desktop",
          command: remoteCommand.trim(),
          argsJson: compactJson(remoteArgsJson),
          workingDir: remoteWorkingDir.trim() || null,
          policyJson: compactJson(remotePolicyJson),
          requestedBy: "desktop",
          requestedAt: timestamp,
        },
      },
    );
    return decision.accepted
      ? `Approved remote command ${decision.command.id}.`
      : decision.message ?? `Rejected remote command ${decision.command.id}.`;
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-background">
      <div className="border-b bg-card/95 px-5 py-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Bot className="h-4 w-4 text-primary" />
              Dock Agent
            </div>
            <p className="mt-1 max-w-2xl text-xs text-muted-foreground">
              Primary orchestration layer for provider tasks, schedules, chat ingress,
              remote command approvals, and audit trails.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {STATUS_FILTERS.map((status) => (
              <button
                key={status}
                type="button"
                className={cn(
                  "h-7 rounded-md border px-2.5 text-xs font-medium transition-colors",
                  taskFilter === status
                    ? "border-primary bg-primary text-primary-foreground"
                    : "border-border bg-background hover:bg-accent",
                )}
                onClick={() => setTaskFilter(status)}
              >
                {status === "all" ? "All" : status}
              </button>
            ))}
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void refresh()}
              disabled={loading}
            >
              {loading ? <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="mr-2 h-3.5 w-3.5" />}
              Refresh
            </Button>
          </div>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(320px,420px)_minmax(0,1fr)] gap-0 overflow-hidden">
        <aside className="min-h-0 overflow-y-auto border-r bg-muted/20 p-4">
          <section className="rounded-lg border bg-card p-4 shadow-sm">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <ClipboardList className="h-4 w-4 text-primary" />
              Queue task
            </div>
            <div className="mt-4 space-y-3">
              <label className="block text-xs font-medium">
                Title
                <input
                  value={taskTitle}
                  onChange={(event) => setTaskTitle(event.target.value)}
                  className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </label>
              <label className="block text-xs font-medium">
                Objective
                <Textarea
                  value={taskObjective}
                  onChange={(event) => setTaskObjective(event.target.value)}
                  className="mt-1 min-h-[104px] resize-none"
                />
              </label>
              <div className="grid grid-cols-2 gap-2">
                <label className="block text-xs font-medium">
                  Provider
                  <select
                    value={taskProviderId}
                    onChange={(event) =>
                      setTaskProviderId(event.target.value as DockAgentProviderId)
                    }
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
                  >
                    {providerOptions()}
                  </select>
                </label>
                <label className="block text-xs font-medium">
                  Thread id
                  <input
                    value={targetThreadId}
                    onChange={(event) => setTargetThreadId(event.target.value)}
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    placeholder="optional"
                  />
                </label>
              </div>
              {!selectedThreadIsMvpProvider && selectedThread ? (
                <p className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300">
                  Selected thread provider is outside the MVP set. Pick Codex, Claude Code, or OpenCode.
                </p>
              ) : null}
              <JsonCodeEditor
                value={taskMetadataJson}
                onChange={setTaskMetadataJson}
                darkMode={darkMode}
                minHeight={96}
              />
              <Button
                type="button"
                className="w-full"
                onClick={() => void runAction("create-task", createTask)}
                disabled={actionLoading !== null}
              >
                {actionLoading === "create-task" ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Play className="mr-2 h-4 w-4" />}
                Queue task
              </Button>
            </div>
          </section>

          <section className="mt-4 rounded-lg border bg-card p-4 shadow-sm">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <CalendarClock className="h-4 w-4 text-primary" />
              Schedule
            </div>
            <div className="mt-4 space-y-3">
              <div className="flex items-center justify-between gap-3 rounded-md border bg-background px-3 py-2">
                <span className="text-xs font-medium">Enabled</span>
                <Switch checked={scheduleEnabled} onCheckedChange={setScheduleEnabled} />
              </div>
              <div className="grid grid-cols-[1fr_1.4fr] gap-2">
                <label className="block text-xs font-medium">
                  Cadence
                  <select
                    value={scheduleCadence}
                    onChange={(event) =>
                      setScheduleCadence(event.target.value as DockAgentScheduleCadence)
                    }
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-2 text-sm outline-none focus:ring-2 focus:ring-ring"
                  >
                    <option value="hourly">Hourly</option>
                    <option value="weekly">Weekly</option>
                    <option value="cron">Cron</option>
                  </select>
                </label>
                <label className="block text-xs font-medium">
                  Cron
                  <input
                    value={scheduleCron}
                    onChange={(event) => setScheduleCron(event.target.value)}
                    disabled={scheduleCadence !== "cron"}
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
                  />
                </label>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => void runAction("create-schedule", createSchedule)}
                  disabled={actionLoading !== null}
                >
                  Create
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => void runAction("enqueue-schedules", enqueueDueSchedules)}
                  disabled={actionLoading !== null}
                >
                  Enqueue due
                </Button>
              </div>
            </div>
          </section>

          <section className="mt-4 rounded-lg border bg-card p-4 shadow-sm">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Webhook className="h-4 w-4 text-primary" />
              Chat ingress
            </div>
            <div className="mt-4 space-y-3">
              <label className="block text-xs font-medium">
                Connector
                <input
                  value={connectorName}
                  onChange={(event) => setConnectorName(event.target.value)}
                  className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </label>
              <label className="block text-xs font-medium">
                Message
                <Textarea
                  value={chatText}
                  onChange={(event) => setChatText(event.target.value)}
                  className="mt-1 min-h-[80px] resize-none"
                />
              </label>
              <Button
                type="button"
                variant="outline"
                className="w-full"
                onClick={() => void runAction("chat-command", sendChatCommand)}
                disabled={actionLoading !== null}
              >
                Send webhook command
              </Button>
            </div>
          </section>

          <section className="mt-4 rounded-lg border bg-card p-4 shadow-sm">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <ShieldCheck className="h-4 w-4 text-primary" />
              Remote command gate
            </div>
            <div className="mt-4 space-y-3">
              <div className="grid grid-cols-[0.8fr_1.2fr] gap-2">
                <label className="block text-xs font-medium">
                  Command
                  <input
                    value={remoteCommand}
                    onChange={(event) => setRemoteCommand(event.target.value)}
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                  />
                </label>
                <label className="block text-xs font-medium">
                  Working dir
                  <input
                    value={remoteWorkingDir}
                    onChange={(event) => setRemoteWorkingDir(event.target.value)}
                    className="mt-1 h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                  />
                </label>
              </div>
              <JsonCodeEditor
                value={remoteArgsJson}
                onChange={setRemoteArgsJson}
                darkMode={darkMode}
                minHeight={82}
              />
              <JsonCodeEditor
                value={remotePolicyJson}
                onChange={setRemotePolicyJson}
                darkMode={darkMode}
                minHeight={110}
              />
              <Button
                type="button"
                variant="outline"
                className="w-full"
                onClick={() => void runAction("remote-command", requestRemoteCommand)}
                disabled={actionLoading !== null}
              >
                Request approval
              </Button>
            </div>
          </section>
        </aside>

        <section className="min-h-0 overflow-y-auto p-4">
          {message ? (
            <div className="mb-3 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-300">
              {message}
            </div>
          ) : null}
          {error ? (
            <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-700 dark:text-red-300">
              {error}
            </div>
          ) : null}

          <div className="grid grid-cols-5 gap-2">
            {Object.entries(taskStats).map(([status, count]) => (
              <div key={status} className="rounded-lg border bg-card px-3 py-2 shadow-sm">
                <p className="text-[11px] uppercase text-muted-foreground">{status}</p>
                <p className="mt-1 text-xl font-semibold tabular-nums">{count}</p>
              </div>
            ))}
          </div>

          <div className="mt-4 grid gap-4 xl:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
            <section className="rounded-lg border bg-card shadow-sm">
              <div className="flex items-center justify-between border-b px-4 py-3">
                <div className="flex items-center gap-2 text-sm font-semibold">
                  <TerminalSquare className="h-4 w-4 text-primary" />
                  Task queue
                </div>
                <span className="text-xs text-muted-foreground">{tasks.length} shown</span>
              </div>
              <div className="divide-y">
                {tasks.length === 0 ? (
                  <div className="px-4 py-10 text-center text-sm text-muted-foreground">
                    No Dock Agent tasks match this filter.
                  </div>
                ) : (
                  tasks.map((task) => (
                    <article key={task.id} className="px-4 py-3">
                      <div className="flex min-w-0 items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex min-w-0 flex-wrap items-center gap-2">
                            <p className="truncate text-sm font-semibold">{task.title}</p>
                            <Badge variant="outline" className={cn("capitalize", statusTone(task.status))}>
                              {task.status}
                            </Badge>
                            <Badge variant="secondary" className="capitalize">
                              {task.kind.replace("_", " ")}
                            </Badge>
                          </div>
                          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                            {task.objective}
                          </p>
                        </div>
                        <div className="shrink-0 text-right text-[11px] text-muted-foreground">
                          <div>{formatShortTime(task.updatedAt)}</div>
                          <div>{task.providerId ? providerDisplayName(task.providerId) : "Dock"}</div>
                        </div>
                      </div>
                      <div className="mt-2 grid grid-cols-3 gap-2 text-[11px] text-muted-foreground">
                        <span className="truncate">id: {task.id}</span>
                        <span className="truncate">thread: {task.targetThreadId ?? "-"}</span>
                        <span className="truncate">schedule: {task.scheduleId ?? "-"}</span>
                      </div>
                    </article>
                  ))
                )}
              </div>
            </section>

            <div className="space-y-4">
              <section className="rounded-lg border bg-card shadow-sm">
                <div className="flex items-center justify-between border-b px-4 py-3">
                  <div className="flex items-center gap-2 text-sm font-semibold">
                    <CalendarClock className="h-4 w-4 text-primary" />
                    Due schedules
                  </div>
                  <span className="text-xs text-muted-foreground">{dueSchedules.length}</span>
                </div>
                <div className="divide-y">
                  {dueSchedules.length === 0 ? (
                    <div className="px-4 py-8 text-sm text-muted-foreground">
                      No schedules are due right now.
                    </div>
                  ) : (
                    dueSchedules.map((schedule) => (
                      <div key={schedule.id} className="px-4 py-3 text-sm">
                        <div className="flex items-center justify-between gap-3">
                          <span className="font-medium capitalize">{schedule.cadence}</span>
                          <Badge variant={schedule.enabled ? "secondary" : "outline"}>
                            {schedule.enabled ? "enabled" : "disabled"}
                          </Badge>
                        </div>
                        <p className="mt-1 truncate text-xs text-muted-foreground">
                          next: {formatShortTime(schedule.nextRunAt)} / last: {formatShortTime(schedule.lastRunAt)}
                        </p>
                      </div>
                    ))
                  )}
                </div>
              </section>

              <section className="rounded-lg border bg-card shadow-sm">
                <div className="flex items-center justify-between border-b px-4 py-3">
                  <div className="flex items-center gap-2 text-sm font-semibold">
                    <History className="h-4 w-4 text-primary" />
                    Audit trail
                  </div>
                  <span className="text-xs text-muted-foreground">{auditLogs.length}</span>
                </div>
                <div className="max-h-[520px] divide-y overflow-y-auto">
                  {auditLogs.length === 0 ? (
                    <div className="px-4 py-8 text-sm text-muted-foreground">
                      No audit events yet.
                    </div>
                  ) : (
                    auditLogs.map((log) => (
                      <div key={log.id} className="px-4 py-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex min-w-0 items-center gap-2">
                            <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-primary" />
                            <span className="truncate text-sm font-medium">{log.action}</span>
                          </div>
                          <Badge variant="outline" className={statusTone(log.outcome)}>
                            {log.outcome}
                          </Badge>
                        </div>
                        <p className="mt-1 truncate text-xs text-muted-foreground">
                          {log.actor} on {log.subjectType}
                          {log.subjectId ? `:${log.subjectId}` : ""} at {formatShortTime(log.createdAt)}
                        </p>
                      </div>
                    ))
                  )}
                </div>
              </section>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
