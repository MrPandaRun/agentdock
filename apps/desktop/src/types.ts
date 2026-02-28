export interface AgentThreadSummary {
  id: string;
  providerId: "claude_code" | string;
  projectPath: string;
  title: string;
  tags: string[];
  lastActiveAt: string;
  lastMessagePreview?: string | null;
}

export interface ProviderInstallStatus {
  providerId: ThreadProviderId;
  installed: boolean;
  healthStatus: "healthy" | "degraded" | "offline" | string;
  message?: string | null;
}


export type ThreadProviderId = "claude_code" | "codex" | "opencode";
export type AppTheme = "light" | "dark" | "system";
export type TerminalTheme = "dark" | "light";
