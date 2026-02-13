import { Bot, Code2, Monitor, PanelLeftClose, PanelLeftOpen, SquareTerminal } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import {
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { AgentThreadSummary, RightPaneMode } from "@/types";
import type { EmbeddedTerminalNewThreadLaunch } from "@/hooks/useThreads";
import {
  isCodexProvider,
  isOpenCodeProvider,
  providerAccentClass,
  providerDisplayName,
} from "@/lib/provider";

export interface ThreadHeaderProps {
  sidebarCollapsed: boolean;
  rightPaneMode: RightPaneMode;
  selectedThread: AgentThreadSummary | null;
  newThreadLaunch: EmbeddedTerminalNewThreadLaunch | null;
  loadingMessages: boolean;
  showToolEvents: boolean;
  toolCount: number;
  displayedMessagesCount: number;
  messagesCount: number;
  onToggleSidebar: () => void;
  onSetRightPaneMode: (mode: RightPaneMode) => void;
  onToggleShowToolEvents: (show: boolean) => void;
}

export function ThreadHeader({
  sidebarCollapsed,
  rightPaneMode,
  selectedThread,
  newThreadLaunch,
  loadingMessages,
  showToolEvents,
  toolCount,
  displayedMessagesCount,
  messagesCount,
  onToggleSidebar,
  onSetRightPaneMode,
  onToggleShowToolEvents,
}: ThreadHeaderProps) {
  const headerProviderId =
    rightPaneMode === "terminal" && newThreadLaunch
      ? newThreadLaunch.providerId
      : selectedThread?.providerId;
  const headerTitle =
    rightPaneMode === "terminal" && newThreadLaunch
      ? `New ${providerDisplayName(newThreadLaunch.providerId)} thread`
      : (selectedThread?.title ?? "Select a thread");
  const headerProjectPath =
    rightPaneMode === "terminal" && newThreadLaunch
      ? newThreadLaunch.projectPath
      : (selectedThread?.projectPath ?? "-");
  const headerProviderName = providerDisplayName(headerProviderId);
  const headerProviderAccent = providerAccentClass(headerProviderId);

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
          <CardTitle className="text-[22px] leading-none">{headerTitle}</CardTitle>
          <CardDescription className="truncate text-xs">
            {headerProjectPath}
          </CardDescription>
        </div>

        <div className="grid justify-items-end gap-1 text-xs text-muted-foreground">
          <span>
            {rightPaneMode === "terminal"
              ? "Embedded terminal"
              : loadingMessages
                ? "Syncing..."
                : "Ready"}
          </span>
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
            <div className="inline-flex items-center rounded-md border border-border/70 bg-muted/35 p-0.5">
              <Button
                type="button"
                variant={rightPaneMode === "terminal" ? "secondary" : "ghost"}
                size="sm"
                className="h-6 gap-1 px-2 text-[11px]"
                onClick={() => onSetRightPaneMode("terminal")}
              >
                <SquareTerminal className="h-3 w-3" />
                Terminal
              </Button>
              <Button
                type="button"
                variant={rightPaneMode === "ui" ? "secondary" : "ghost"}
                size="sm"
                className="h-6 gap-1 px-2 text-[11px]"
                onClick={() => onSetRightPaneMode("ui")}
              >
                <Monitor className="h-3 w-3" />
                UI
                <span className="ml-0.5 text-[8px] lowercase text-muted-foreground/65">
                  alpha
                </span>
              </Button>
            </div>
            {rightPaneMode === "ui" ? (
              <>
                <Switch
                  id="show-tool-events"
                  checked={showToolEvents}
                  onCheckedChange={onToggleShowToolEvents}
                />
                <label htmlFor="show-tool-events" className="cursor-pointer">
                  Show steps
                </label>
                {toolCount > 0 ? (
                  <Badge variant="outline" className="h-5 px-2 text-[10px]">
                    {toolCount}
                  </Badge>
                ) : null}
              </>
            ) : null}
          </div>
          {rightPaneMode === "ui" ? (
            <span>
              {displayedMessagesCount}/{messagesCount} messages
            </span>
          ) : (
            <span className="text-[11px]">Interactive CLI in-app</span>
          )}
        </div>
      </div>
    </CardHeader>
  );
}
