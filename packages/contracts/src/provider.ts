export type ProviderId = "codex" | "claude_code" | "opencode";

export type ProviderErrorCode =
  | "credential_missing"
  | "credential_expired"
  | "permission_denied"
  | "timeout"
  | "upstream_unavailable"
  | "invalid_response"
  | "not_implemented"
  | "unknown";

export interface ProviderError {
  code: ProviderErrorCode;
  message: string;
  retryable: boolean;
}

export type ProviderHealthStatus = "healthy" | "degraded" | "offline";

export interface ProviderHealthCheckRequest {
  profileName: string;
  projectPath?: string;
}

export interface ProviderHealthCheckResult {
  providerId: ProviderId;
  status: ProviderHealthStatus;
  checkedAt: string;
  message?: string;
}

export interface ThreadSummary {
  id: string;
  providerId: ProviderId;
  accountId?: string;
  projectPath: string;
  title: string;
  tags: string[];
  lastActiveAt: string;
}

export interface ResumeThreadRequest {
  threadId: string;
  projectPath?: string;
}

export interface ResumeThreadResult {
  threadId: string;
  resumed: boolean;
  message?: string;
}

export interface ProviderAdapter {
  readonly providerId: ProviderId;
  healthCheck(
    request: ProviderHealthCheckRequest,
  ): Promise<ProviderHealthCheckResult>;
  listThreads(projectPath?: string): Promise<ThreadSummary[]>;
  resumeThread(request: ResumeThreadRequest): Promise<ResumeThreadResult>;
}

export const SUPPORTED_PROVIDERS: ProviderId[] = ["codex", "claude_code", "opencode"];
