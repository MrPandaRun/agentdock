import {
  Bot,
  Check,
  ChevronDown,
  Code2,
  GitBranch,
  Loader2,
  PanelLeftClose,
  PanelLeftOpen,
  SquareTerminal,
  Wrench,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type {
  EmbeddedTerminalNewThreadLaunch,
  NewThreadBindingStatus,
} from "@/hooks/useThreads";
import type {
  AgentThreadSummary,
  OpenTargetId,
  OpenTargetStatus,
  ProjectGitBranchInfo,
} from "@/types";
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
  openTargets: OpenTargetStatus[];
  loadingOpenTargets: boolean;
  defaultOpenTargetId: OpenTargetId;
  quickOpenTargetId: OpenTargetId;
  openingTargetId: OpenTargetId | null;
  openTargetError: string | null;
  onOpenWithTarget: (targetId: OpenTargetId) => Promise<void>;
  onDefaultOpenTargetChange: (targetId: OpenTargetId) => void;
  ideContextEnabled: boolean;
  ideContextToggleDisabled: boolean;
  onIdeContextEnabledChange: (enabled: boolean) => void;
  gitBranchInfo: ProjectGitBranchInfo | null;
  gitBranchLoading: boolean;
  onToggleSidebar: () => void;
}

function formatGitBranchText(
  gitBranchInfo: ProjectGitBranchInfo | null,
  gitBranchLoading: boolean,
): string {
  if (gitBranchLoading) {
    return "Git: Checking...";
  }
  if (!gitBranchInfo) {
    return "Git: -";
  }

  if (gitBranchInfo.status === "ok") {
    return `Git: ${gitBranchInfo.branch ?? "-"}`;
  }
  if (gitBranchInfo.status === "no_repo") {
    return "Git: No git repo";
  }
  if (gitBranchInfo.status === "path_missing") {
    return "Git: Path missing";
  }
  return "Git: Unavailable";
}

export function ThreadHeader({
  sidebarCollapsed,
  selectedThread,
  newThreadLaunch,
  newThreadBindingStatus,
  openTargets,
  loadingOpenTargets,
  defaultOpenTargetId,
  quickOpenTargetId,
  openingTargetId,
  openTargetError,
  onOpenWithTarget,
  onDefaultOpenTargetChange,
  ideContextEnabled,
  ideContextToggleDisabled,
  onIdeContextEnabledChange,
  gitBranchInfo,
  gitBranchLoading,
  onToggleSidebar,
}: ThreadHeaderProps) {
  const [devMenuOpen, setDevMenuOpen] = useState(false);
  const devButtonRef = useRef<HTMLButtonElement | null>(null);
  const devPanelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!devMenuOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) {
        return;
      }
      if (devPanelRef.current?.contains(target)) {
        return;
      }
      if (devButtonRef.current?.contains(target)) {
        return;
      }
      setDevMenuOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setDevMenuOpen(false);
      }
    };

    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [devMenuOpen]);

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
        : null;
  const gitBranchText = formatGitBranchText(gitBranchInfo, gitBranchLoading);
  const hasValidProjectPath = headerProjectPath.trim().length > 0 && headerProjectPath !== "-";
  const defaultOpenTarget = useMemo(
    () =>
      openTargets.find((target) => target.id === defaultOpenTargetId) ??
      openTargets.find((target) => target.available) ??
      null,
    [defaultOpenTargetId, openTargets],
  );
  const quickOpenTarget = useMemo(
    () =>
      openTargets.find((target) => target.id === quickOpenTargetId) ??
      defaultOpenTarget,
    [defaultOpenTarget, openTargets, quickOpenTargetId],
  );
  const openingQuickTarget = quickOpenTarget
    ? openingTargetId === quickOpenTarget.id
    : false;

  const handleQuickOpen = () => {
    if (!quickOpenTarget) {
      return;
    }
    void onOpenWithTarget(quickOpenTarget.id);
  };

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

        <div className="grid justify-items-end gap-2 text-xs text-muted-foreground">
          {terminalStatusText ? <span>{terminalStatusText}</span> : null}
          <span className="inline-flex items-center gap-1 text-[11px]">
            <GitBranch className="h-3 w-3" />
            {gitBranchText}
          </span>
          <div className="flex items-center gap-2">
            <div className="relative">
              <div className="flex items-center">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 rounded-r-none border-r-0 px-2 text-[11px]"
                  disabled={
                    !quickOpenTarget ||
                    !quickOpenTarget.installed ||
                    !quickOpenTarget.available ||
                    !hasValidProjectPath ||
                    openingTargetId !== null
                  }
                  onClick={handleQuickOpen}
                >
                  {openingQuickTarget ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : (
                    <Wrench className="h-3.5 w-3.5" />
                  )}
                  Quick Open {quickOpenTarget?.label ?? ""}
                </Button>
                <Button
                  ref={devButtonRef}
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 w-7 rounded-l-none px-0"
                  onClick={() => setDevMenuOpen((open) => !open)}
                  aria-expanded={devMenuOpen}
                  aria-haspopup="menu"
                  aria-label="Open target menu"
                >
                  <ChevronDown className="h-3.5 w-3.5" />
                </Button>
              </div>

              {devMenuOpen ? (
                <div
                  ref={devPanelRef}
                  className="absolute right-0 top-8 z-40 w-[280px] rounded-md border border-border bg-card p-1.5 shadow-md"
                  role="menu"
                >
                  <div className="max-h-44 space-y-0.5 overflow-y-auto">
                    {loadingOpenTargets ? (
                      <div className="px-2 py-1.5 text-[11px] text-muted-foreground">
                        Detecting open targets...
                      </div>
                    ) : openTargets.length === 0 ? (
                      <div className="px-2 py-1.5 text-[11px] text-muted-foreground">
                        No targets detected.
                      </div>
                    ) : (
                      openTargets.map((target) => {
                        const disabled =
                          !target.installed ||
                          !target.available ||
                          !hasValidProjectPath ||
                          openingTargetId !== null;
                        return (
                          <Button
                            key={target.id}
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="h-8 w-full justify-between px-2 text-xs"
                            disabled={disabled}
                            onClick={() => {
                              setDevMenuOpen(false);
                              onDefaultOpenTargetChange(target.id);
                              void onOpenWithTarget(target.id);
                            }}
                            role="menuitem"
                          >
                            <span className="inline-flex items-center gap-1.5">
                              {openingTargetId === target.id ? (
                                <Loader2 className="h-3 w-3 animate-spin" />
                              ) : target.id === defaultOpenTargetId ? (
                                <Check className="h-3 w-3" />
                              ) : (
                                <span className="inline-block h-3 w-3" />
                              )}
                              {target.label}
                            </span>
                            <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
                              {!target.installed
                                ? "Not installed"
                                : !target.available
                                  ? "Unavailable"
                                  : target.id === quickOpenTargetId
                                    ? "Last used"
                                    : target.kind}
                            </span>
                          </Button>
                        );
                      })
                    )}
                  </div>

                  <div className="my-1 h-px bg-border" />

                  <div className="flex items-center justify-between px-2 py-1">
                    <div className="space-y-0.5 text-left">
                      <p className="text-xs font-medium text-foreground">IDE Context</p>
                      <p className="text-[11px] text-muted-foreground">
                        {ideContextEnabled ? "Connected" : "Disconnected"}
                      </p>
                    </div>
                    <Switch
                      checked={ideContextEnabled}
                      onCheckedChange={onIdeContextEnabledChange}
                      disabled={ideContextToggleDisabled}
                    />
                  </div>
                  {ideContextToggleDisabled ? (
                    <p className="px-2 pb-1 text-[11px] text-muted-foreground">
                      Available for saved threads only.
                    </p>
                  ) : null}
                </div>
              ) : null}
            </div>
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
          {openTargetError ? (
            <span className="max-w-[280px] truncate text-[11px] text-rose-500">
              {openTargetError}
            </span>
          ) : null}
        </div>
      </div>
    </CardHeader>
  );
}
