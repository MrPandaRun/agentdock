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
    return "text-[hsl(var(--brand-codex))]";
  }
  if (isOpenCodeProvider(providerId)) {
    return "text-[hsl(var(--brand-opencode))]";
  }
  return "text-[hsl(var(--brand-claude))]";
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
