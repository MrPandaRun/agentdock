import type { ThreadProviderId } from "@/types";

export function isCodexProvider(providerId?: string): boolean {
  return providerId === "codex";
}

export function isOpenCodeProvider(providerId?: string): boolean {
  return providerId === "opencode";
}

export function providerDisplayName(providerId?: string): string {
  if (providerId === "codex") {
    return "Codex";
  }
  if (providerId === "opencode") {
    return "OpenCode";
  }
  if (providerId === "claude_code") {
    return "Claude Code";
  }
  return providerId ?? "Provider";
}

export function providerAccentClass(providerId?: string): string {
  if (isCodexProvider(providerId)) {
    return "text-[#111111] dark:text-[#F2F2F2]";
  }
  if (isOpenCodeProvider(providerId)) {
    return "text-[#211E1E] dark:text-[#F1ECEC]";
  }
  return "text-[#FF7043] dark:text-[#FF8A65]";
}

export function isSupportedProvider(value: string): value is ThreadProviderId {
  return (
    value === "claude_code" || value === "codex" || value === "opencode"
  );
}

export function providerInstallGuideUrl(providerId: ThreadProviderId): string {
  if (providerId === "claude_code") {
    return "https://docs.anthropic.com/en/docs/claude-code/overview";
  }
  if (providerId === "codex") {
    return "https://platform.openai.com/docs/codex";
  }
  return "https://opencode.ai/docs";
}
