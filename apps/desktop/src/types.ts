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

export type OpenTargetId =
  | "vscode"
  | "cursor"
  | "windsurf"
  | "antigravity"
  | "zed"
  | "intellij"
  | "webstorm"
  | "pycharm"
  | "sublime_text"
  | "terminal"
  | "iterm"
  | "warp";

export interface OpenTargetStatus {
  id: OpenTargetId;
  label: string;
  installed: boolean;
  available: boolean;
  detail?: string | null;
  kind: "ide" | "terminal";
}

export type ProjectGitBranchStatus = "ok" | "no_repo" | "path_missing" | "error";

export interface ProjectGitBranchInfo {
  status: ProjectGitBranchStatus;
  branch?: string | null;
  message?: string | null;
}

export type AgentSupplierKind = "official" | "custom";

export interface AgentSupplier {
  id: string;
  kind: AgentSupplierKind;
  name: string;
  note?: string;
  profileName: string;
  baseUrl?: string;
  apiKey?: string;
  configJson?: string;
  updatedAt: number;
}

export type ProviderProfileMap = Record<ThreadProviderId, string>;

export interface ActiveAgentProfileSelection {
  activeProviderId: ThreadProviderId;
  profiles: ProviderProfileMap;
}

export type AgentSupplierMap = Record<ThreadProviderId, AgentSupplier[]>;

export interface AgentRuntimeSettings {
  activeProviderId: ThreadProviderId;
  activeSupplierIds: Record<ThreadProviderId, string>;
  suppliersByProvider: AgentSupplierMap;
}

export type ThreadProviderId = "claude_code" | "codex" | "opencode";
export type AppTheme = "light" | "dark" | "system";
export type TerminalTheme = "dark" | "light";

export interface SkillEnabledState {
  claude_code: boolean;
  codex: boolean;
  opencode: boolean;
}

export interface Skill {
  id: string;
  name: string;
  description?: string;
  source: string;
  version: string;
  enabledJson: string;
  compatibilityJson: string;
  readmeUrl?: string;
  repoOwner?: string;
  repoName?: string;
  repoBranch?: string;
  installedAt: number;
}

export interface SkillRepo {
  id: string;
  owner: string;
  name: string;
  branch: string;
  enabled: boolean;
  createdAt: number;
}

export interface DiscoverableSkill {
  key: string;
  name: string;
  description: string;
  directory: string;
  readmeUrl?: string;
  repoOwner: string;
  repoName: string;
  repoBranch: string;
}

export type DiscoverSkillInstallStage =
  | "queued"
  | "downloading"
  | "extracting"
  | "parsing_metadata"
  | "saving_record"
  | "syncing_files"
  | "syncing_providers"
  | "completed"
  | "failed";

export interface DiscoverSkillInstallProgress {
  key: string;
  stage: DiscoverSkillInstallStage | string;
  message: string;
}

export interface ProviderSkill {
  key: string;
  name: string;
  description: string;
  directory: string;
  provider: string;
  path: string;
}
