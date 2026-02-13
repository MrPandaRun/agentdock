export interface AgentThreadSummary {
  id: string;
  providerId: "claude_code" | string;
  projectPath: string;
  title: string;
  tags: string[];
  lastActiveAt: string;
  lastMessagePreview?: string | null;
}

export interface AgentThreadMessage {
  role: string;
  content: string;
  timestampMs?: number;
  kind: "text" | "tool" | string;
  collapsed: boolean;
}

export interface SendClaudeMessageResponse {
  threadId: string;
  responseText: string;
  rawOutput: string;
}

export interface ToolMessageParts {
  headline: string;
  detail?: string;
  ioLabel?: "IN" | "OUT";
  ioBody?: string;
}

export type RightPaneMode = "terminal" | "ui";
export type ThreadProviderId = "claude_code" | "codex" | "opencode";
