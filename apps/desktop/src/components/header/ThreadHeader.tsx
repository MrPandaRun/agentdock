import {
  Bot,
  Code2,
  PanelLeftClose,
  PanelLeftOpen,
  SquareTerminal,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type {
  EmbeddedTerminalNewThreadLaunch,
  NewThreadBindingStatus,
} from "@/hooks/useThreads";
import type { AgentThreadSummary } from "@/types";
import {
  isCodexProvider,
  isOpenCodeProvider,
  providerAccentClass,
  providerDisplayName,
} from "@/lib/provider";

export interface ThreadHeaderProps {
  sidebarCollapsed: boolean;
  selectedThread: AgentThreadSummary | null;
  newThreadLaunch: EmbeddedTerminalNewThreadLaunch | null;
  newThreadBindingStatus: NewThreadBindingStatus | null;
  onToggleSidebar: () => void;
}

export function ThreadHeader({
  sidebarCollapsed,
  selectedThread,
  newThreadLaunch,
  newThreadBindingStatus,
  onToggleSidebar,
}: ThreadHeaderProps) {
  const headerProviderId = newThreadLaunch
    ? newThreadLaunch.providerId
    : selectedThread?.providerId;
  const headerTitle = newThreadLaunch
    ? `New ${providerDisplayName(newThreadLaunch.providerId)} thread`
    : (selectedThread?.title ?? "Select a thread");
  const headerProjectPath = newThreadLaunch
    ? newThreadLaunch.projectPath
    : (selectedThread?.projectPath ?? "-");
  const headerProviderName = providerDisplayName(headerProviderId);
  const headerProviderAccent = providerAccentClass(headerProviderId);
  const terminalStatusText =
    newThreadBindingStatus === "starting"
      ? "Starting new session..."
      : newThreadBindingStatus === "awaiting_discovery"
        ? "Session running. Waiting for first input to persist thread id..."
        : "Embedded terminal";

  return (
    <CardHeader className="px-4 py-3 pb-2.5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="space-y-1">
          <p
            className={cn(
              "inline-flex items-center gap-1.5 text-[11px] uppercase tracking-[0.16em]",
              headerProviderAccent,
            )}
          >
            {isCodexProvider(headerProviderId) ? (
              <SquareTerminal className="h-3.5 w-3.5" />
            ) : isOpenCodeProvider(headerProviderId) ? (
              <Code2 className="h-3.5 w-3.5" />
            ) : (
              <Bot className="h-3.5 w-3.5" />
            )}
            {headerProviderName}
          </p>
          <CardTitle className="text-[22px] leading-none">
            {headerTitle}
          </CardTitle>
          <CardDescription className="truncate text-xs">
            {headerProjectPath}
          </CardDescription>
        </div>

        <div className="grid justify-items-end gap-1 text-xs text-muted-foreground">
          <span>{terminalStatusText}</span>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-7 w-7 rounded-md"
              onClick={onToggleSidebar}
            >
              {sidebarCollapsed ? (
                <PanelLeftOpen className="h-3.5 w-3.5" />
              ) : (
                <PanelLeftClose className="h-3.5 w-3.5" />
              )}
            </Button>
          </div>
          <span className="text-[11px]">Interactive CLI in-app</span>
        </div>
      </div>
    </CardHeader>
  );
}
