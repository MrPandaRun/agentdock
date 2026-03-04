import claudeCodeSvgRaw from "@/assets/providers/claude-code.svg?raw";
import openAiSvgRaw from "@/assets/providers/openai.svg?raw";
import openCodeSvgRaw from "@/assets/providers/opencode.svg?raw";
import { isSupportedProvider } from "@/lib/provider";
import { cn } from "@/lib/utils";

import type { ThreadProviderId } from "@/types";

const PROVIDER_ICON_SVG_BY_ID: Record<ThreadProviderId, string> = {
  claude_code: claudeCodeSvgRaw,
  codex: openAiSvgRaw,
  opencode: openCodeSvgRaw,
};

function normalizeSvgMarkup(raw: string): string {
  return raw
    .replace(/\swidth="[^"]*"/g, "")
    .replace(/\sheight="[^"]*"/g, "");
}

const NORMALIZED_PROVIDER_ICON_SVG_BY_ID: Record<ThreadProviderId, string> = {
  claude_code: normalizeSvgMarkup(PROVIDER_ICON_SVG_BY_ID.claude_code),
  codex: normalizeSvgMarkup(PROVIDER_ICON_SVG_BY_ID.codex),
  opencode: normalizeSvgMarkup(PROVIDER_ICON_SVG_BY_ID.opencode),
};

function providerIconColorClass(providerId: ThreadProviderId): string {
  if (providerId === "claude_code") {
    return "text-[#FF7043] dark:text-[#FF8A65]";
  }
  if (providerId === "codex") {
    return "text-[#111111] dark:text-[#F2F2F2]";
  }
  return "text-[#211E1E] dark:text-[#F1ECEC]";
}

export interface ProviderIconProps {
  providerId?: string;
  className?: string;
}

export function ProviderIcon({ providerId, className }: ProviderIconProps) {
  const normalizedProviderId: ThreadProviderId =
    providerId && isSupportedProvider(providerId) ? providerId : "claude_code";
  const iconSvg = NORMALIZED_PROVIDER_ICON_SVG_BY_ID[normalizedProviderId];

  return (
    <span
      className={cn("inline-flex shrink-0", className)}
      aria-hidden
    >
      <span
        className={cn(
          "[&>svg]:h-full [&>svg]:w-full",
          providerIconColorClass(normalizedProviderId),
        )}
        dangerouslySetInnerHTML={{ __html: iconSvg }}
      />
    </span>
  );
}
