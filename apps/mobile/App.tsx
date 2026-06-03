import { StatusBar } from "expo-status-bar";
import { SafeAreaView, ScrollView, StyleSheet, Text, View } from "react-native";
import type { ReactNode } from "react";

import type {
  DockAgentAuditLog,
  DockAgentRemoteCommand,
  DockAgentSchedule,
  DockAgentTask,
} from "@agentdock/contracts/dock-agent";
import type { ProviderId } from "@agentdock/contracts/provider";

const supportedProviders: ProviderId[] = ["codex", "claude_code", "opencode"];

const activeTasks: DockAgentTask[] = [
  {
    id: "task-mobile-review",
    kind: "orchestration",
    status: "queued",
    title: "Review stale work",
    objective: "Inspect worker threads and resume the highest-priority blocked task.",
    providerId: "codex",
    targetThreadId: "thread-1",
    createdAt: "2026-06-03T00:00:00Z",
    updatedAt: "2026-06-03T00:00:00Z",
    metadataJson: "{}",
  },
  {
    id: "task-mobile-standup",
    kind: "scheduled_check",
    status: "running",
    title: "Morning orchestration check",
    objective: "Collect provider health, queued work, and pending approvals.",
    providerId: "claude_code",
    createdAt: "2026-06-03T01:00:00Z",
    updatedAt: "2026-06-03T01:05:00Z",
    metadataJson: "{}",
  },
];

const schedules: DockAgentSchedule[] = [
  {
    id: "schedule-hourly-review",
    taskId: "task-mobile-review",
    cadence: "hourly",
    timezone: "Asia/Shanghai",
    enabled: true,
    nextRunAt: "2026-06-03T02:00:00Z",
    createdAt: "2026-06-03T00:00:00Z",
    updatedAt: "2026-06-03T00:00:00Z",
  },
];

const remoteCommands: DockAgentRemoteCommand[] = [
  {
    id: "remote-git-status",
    status: "approved",
    command: "git",
    argsJson: "[\"status\",\"--short\"]",
    workingDir: "/workspace/agentdock",
    policyJson: "{\"allowedCommands\":[\"git\"]}",
    requestedBy: "mobile",
    resultJson: "{}",
    createdAt: "2026-06-03T01:10:00Z",
    updatedAt: "2026-06-03T01:10:00Z",
  },
];

const auditLogs: DockAgentAuditLog[] = [
  {
    id: 1,
    actor: "mobile",
    action: "remote_command.approved",
    subjectType: "dock_agent_remote_command",
    subjectId: "remote-git-status",
    outcome: "allowed",
    detailsJson: "{}",
    createdAt: "2026-06-03T01:10:00Z",
  },
];

function providerLabel(providerId: ProviderId): string {
  if (providerId === "claude_code") {
    return "Claude Code";
  }
  if (providerId === "opencode") {
    return "OpenCode";
  }
  return "Codex";
}

function formatProviderScope(): string {
  return supportedProviders.map(providerLabel).join(" / ");
}

export default function App() {
  return (
    <SafeAreaView style={styles.safeArea}>
      <StatusBar style="light" />
      <ScrollView
        style={styles.container}
        contentContainerStyle={styles.content}
        showsVerticalScrollIndicator={false}
      >
        <View style={styles.hero}>
          <Text style={styles.eyebrow}>AgentDock Remote</Text>
          <Text style={styles.title}>Dock Agent command center</Text>
          <Text style={styles.subtitle}>
            Queue orchestration tasks, monitor schedule runs, and approve constrained
            remote commands from the phone surface.
          </Text>
        </View>

        <View style={styles.providerBand}>
          <Text style={styles.sectionLabel}>MVP providers</Text>
          <Text style={styles.providerText}>{formatProviderScope()}</Text>
        </View>

        <View style={styles.metricGrid}>
          <MetricCard label="Queued" value="1" tone="amber" />
          <MetricCard label="Running" value="1" tone="blue" />
          <MetricCard label="Approvals" value="1" tone="green" />
        </View>

        <Section title="Active tasks">
          {activeTasks.map((task) => (
            <View key={task.id} style={styles.row}>
              <View style={styles.rowMain}>
                <Text style={styles.rowTitle}>{task.title}</Text>
                <Text style={styles.rowMeta} numberOfLines={2}>
                  {task.objective}
                </Text>
              </View>
              <StatusPill label={task.status} />
            </View>
          ))}
        </Section>

        <Section title="Schedules">
          {schedules.map((schedule) => (
            <View key={schedule.id} style={styles.row}>
              <View style={styles.rowMain}>
                <Text style={styles.rowTitle}>{schedule.cadence} check</Text>
                <Text style={styles.rowMeta}>
                  {schedule.timezone} / next {schedule.nextRunAt ?? "not set"}
                </Text>
              </View>
              <StatusPill label={schedule.enabled ? "enabled" : "disabled"} />
            </View>
          ))}
        </Section>

        <Section title="Remote command gate">
          {remoteCommands.map((command) => (
            <View key={command.id} style={styles.commandBox}>
              <View style={styles.row}>
                <View style={styles.rowMain}>
                  <Text style={styles.rowTitle}>{command.command}</Text>
                  <Text style={styles.rowMeta}>{command.workingDir ?? "no working dir"}</Text>
                </View>
                <StatusPill label={command.status} />
              </View>
              <Text style={styles.codeText}>{command.argsJson}</Text>
            </View>
          ))}
        </Section>

        <Section title="Audit trail">
          {auditLogs.map((log) => (
            <View key={log.id} style={styles.auditRow}>
              <Text style={styles.rowTitle}>{log.action}</Text>
              <Text style={styles.rowMeta}>
                {log.actor} / {log.outcome} / {log.createdAt}
              </Text>
            </View>
          ))}
        </Section>
      </ScrollView>
    </SafeAreaView>
  );
}

function MetricCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "amber" | "blue" | "green";
}) {
  return (
    <View style={[styles.metricCard, metricToneStyles[tone]]}>
      <Text style={styles.metricValue}>{value}</Text>
      <Text style={styles.metricLabel}>{label}</Text>
    </View>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View style={styles.section}>
      <Text style={styles.sectionTitle}>{title}</Text>
      <View style={styles.sectionBody}>{children}</View>
    </View>
  );
}

function StatusPill({ label }: { label: string }) {
  return (
    <View style={styles.pill}>
      <Text style={styles.pillText}>{label}</Text>
    </View>
  );
}

const metricToneStyles = StyleSheet.create({
  amber: {
    borderColor: "#A16207",
  },
  blue: {
    borderColor: "#0284C7",
  },
  green: {
    borderColor: "#059669",
  },
});

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: "#101820",
  },
  container: {
    flex: 1,
  },
  content: {
    padding: 20,
    paddingBottom: 36,
  },
  hero: {
    paddingTop: 8,
    paddingBottom: 20,
  },
  eyebrow: {
    color: "#90D4D8",
    fontSize: 12,
    fontWeight: "700",
    letterSpacing: 0,
    textTransform: "uppercase",
  },
  title: {
    marginTop: 8,
    color: "#F8FAFC",
    fontSize: 34,
    fontWeight: "800",
    letterSpacing: 0,
    lineHeight: 38,
  },
  subtitle: {
    marginTop: 12,
    color: "#B7C8D6",
    fontSize: 15,
    lineHeight: 22,
  },
  providerBand: {
    borderWidth: 1,
    borderColor: "#2B4654",
    backgroundColor: "#162632",
    borderRadius: 12,
    padding: 14,
  },
  sectionLabel: {
    color: "#88A8B8",
    fontSize: 11,
    fontWeight: "700",
    letterSpacing: 0,
    textTransform: "uppercase",
  },
  providerText: {
    marginTop: 6,
    color: "#F8FAFC",
    fontSize: 15,
    fontWeight: "700",
  },
  metricGrid: {
    flexDirection: "row",
    gap: 10,
    marginTop: 14,
  },
  metricCard: {
    flex: 1,
    borderTopWidth: 3,
    backgroundColor: "#E9F1F4",
    borderRadius: 10,
    padding: 12,
  },
  metricValue: {
    color: "#101820",
    fontSize: 26,
    fontWeight: "800",
  },
  metricLabel: {
    marginTop: 2,
    color: "#425466",
    fontSize: 12,
    fontWeight: "700",
  },
  section: {
    marginTop: 18,
  },
  sectionTitle: {
    color: "#DCEAF1",
    fontSize: 13,
    fontWeight: "800",
    letterSpacing: 0,
    textTransform: "uppercase",
  },
  sectionBody: {
    marginTop: 10,
    gap: 10,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    borderWidth: 1,
    borderColor: "#2B4654",
    backgroundColor: "#162632",
    borderRadius: 12,
    padding: 14,
  },
  rowMain: {
    flex: 1,
    minWidth: 0,
  },
  rowTitle: {
    color: "#F8FAFC",
    fontSize: 15,
    fontWeight: "800",
  },
  rowMeta: {
    marginTop: 4,
    color: "#A7BAC7",
    fontSize: 12,
    lineHeight: 17,
  },
  pill: {
    borderWidth: 1,
    borderColor: "#67E8F9",
    borderRadius: 999,
    paddingHorizontal: 10,
    paddingVertical: 5,
  },
  pillText: {
    color: "#CFFAFE",
    fontSize: 11,
    fontWeight: "800",
  },
  commandBox: {
    borderWidth: 1,
    borderColor: "#2B4654",
    backgroundColor: "#162632",
    borderRadius: 12,
    overflow: "hidden",
  },
  codeText: {
    borderTopWidth: 1,
    borderTopColor: "#2B4654",
    backgroundColor: "#0D151C",
    color: "#C7F9CC",
    fontSize: 12,
    padding: 12,
  },
  auditRow: {
    borderLeftWidth: 3,
    borderLeftColor: "#67E8F9",
    backgroundColor: "#162632",
    borderRadius: 10,
    padding: 12,
  },
});
