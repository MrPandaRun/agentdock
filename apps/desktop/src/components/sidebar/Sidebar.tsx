import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  Bot,
  Check,
  ChevronRight,
  ChevronUp,
  Code2,
  ExternalLink,
  Folder,
  FolderSearch,
  Loader2,
  Monitor,
  Moon,
  Plus,
  RefreshCw,
  Settings2,
  SquareTerminal,
  Sun,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ThreadFolderGroup,
  type ThreadFolderGroupItem,
} from "@/components/threads/ThreadFolderGroup";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  isSupportedProvider,
  providerDisplayName,
  providerInstallGuideUrl,
} from "@/lib/provider";
import { normalizeProjectPath, formatLastActive, threadPreview } from "@/lib/thread";
import { cn } from "@/lib/utils";
import type { AppTheme, ProviderInstallStatus, ThreadProviderId } from "@/types";

interface AppThemeOption {
  value: AppTheme;
  label: string;
  Icon: typeof Sun;
}

interface ProviderOption {
  value: ThreadProviderId;
  label: string;
  Icon: typeof Sun;
  accentClass: string;
}

interface ProviderInstallStatusPayload {
  providerId: string;
  installed: boolean;
  healthStatus: string;
  message?: string | null;
}

const APP_THEME_OPTIONS: AppThemeOption[] = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
];

const THREAD_PROVIDER_OPTIONS: ProviderOption[] = [
  {
    value: "claude_code",
    label: "Claude Code",
    Icon: Bot,
    accentClass: "text-[hsl(var(--brand-claude))]",
  },
  {
    value: "codex",
    label: "Codex",
    Icon: SquareTerminal,
    accentClass: "text-[hsl(var(--brand-codex))]",
  },
  {
    value: "opencode",
    label: "OpenCode",
    Icon: Code2,
    accentClass: "text-[hsl(var(--brand-opencode))]",
  },
];

function emptyProviderInstallStatusMap(): Record<ThreadProviderId, ProviderInstallStatus | null> {
  return {
    claude_code: null,
    codex: null,
    opencode: null,
  };
}

function resolvePickedDirectory(
  picked: string | string[] | null,
): string | null {
  if (typeof picked === "string") {
    return picked;
  }
  if (Array.isArray(picked)) {
    const first = picked[0];
    return typeof first === "string" ? first : null;
  }
  return null;
}

function sanitizeProjectPath(path: string): string {
  const normalized = normalizeProjectPath(path);
  return normalized === "." ? "" : normalized;
}

function providerStatusLabel(
  status: ProviderInstallStatus | null,
  loading: boolean,
): string {
  if (loading && !status) {
    return "Checking...";
  }
  if (!status) {
    return "Unknown";
  }
  if (!status.installed) {
    return "Not Installed";
  }
  return status.healthStatus === "degraded" ? "Installed (Setup Needed)" : "Installed";
}

function providerStatusClass(
  status: ProviderInstallStatus | null,
  loading: boolean,
): string {
  if (loading && !status) {
    return "text-muted-foreground";
  }
  if (!status || !status.installed) {
    return "text-destructive";
  }
  if (status.healthStatus === "degraded") {
    return "text-amber-600 dark:text-amber-400";
  }
  return "text-emerald-600 dark:text-emerald-400";
}

export interface SidebarProps {
  sidebarCollapsed: boolean;
  folderGroups: ThreadFolderGroupItem[];
  selectedFolderKey: string | null;
  selectedThreadId: string | null;
  loadingThreads: boolean;
  creatingThreadFolderKey: string | null;
  error: string | null;
  newThreadBindingStatus: "starting" | "awaiting_discovery" | null;
  hasPendingNewThreadLaunch: boolean;
  appTheme: AppTheme;
  onLoadThreads: () => void;
  onSelectThread: (threadId: string) => void;
  onCreateThread: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
  onAppThemeChange: (theme: AppTheme) => void;
  onClearError: () => void;
}

export function Sidebar({
  sidebarCollapsed,
  folderGroups,
  selectedFolderKey,
  selectedThreadId,
  loadingThreads,
  creatingThreadFolderKey,
  error,
  newThreadBindingStatus,
  hasPendingNewThreadLaunch,
  appTheme,
  onLoadThreads,
  onSelectThread,
  onCreateThread,
  onAppThemeChange,
  onClearError,
}: SidebarProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [themeDialogOpen, setThemeDialogOpen] = useState(false);
  const [pendingTheme, setPendingTheme] = useState<AppTheme>(appTheme);

  const [newThreadDialogOpen, setNewThreadDialogOpen] = useState(false);
  const [selectedProjectPath, setSelectedProjectPath] = useState("");
  const [selectedProviderId, setSelectedProviderId] =
    useState<ThreadProviderId>("claude_code");
  const [isPickingFolder, setIsPickingFolder] = useState(false);
  const [didAttemptCreate, setDidAttemptCreate] = useState(false);
  const [createRequested, setCreateRequested] = useState(false);
  const [didObserveLaunchState, setDidObserveLaunchState] = useState(false);
  const [pickerError, setPickerError] = useState<string | null>(null);
  const [createDialogError, setCreateDialogError] = useState<string | null>(null);
  const [providerInstallStatuses, setProviderInstallStatuses] = useState<
    Record<ThreadProviderId, ProviderInstallStatus | null>
  >(emptyProviderInstallStatusMap);
  const [providerStatusLoading, setProviderStatusLoading] = useState(false);
  const [providerStatusError, setProviderStatusError] = useState<string | null>(null);
  const [providerInstallGuideError, setProviderInstallGuideError] = useState<string | null>(null);

  const settingsRef = useRef<HTMLDivElement | null>(null);

  const folderKeys = useMemo(
    () => new Set(folderGroups.map((group) => group.key)),
    [folderGroups],
  );

  const hasLaunchInFlight = hasPendingNewThreadLaunch || newThreadBindingStatus !== null;
  const isCreateBusy = isPickingFolder || createRequested || hasLaunchInFlight;

  const selectedPathValue = sanitizeProjectPath(selectedProjectPath);
  const hasSelectedFolderInList = folderKeys.has(selectedPathValue);
  const selectedProviderInstallStatus = providerInstallStatuses[selectedProviderId];
  const selectedProviderInstalled = selectedProviderInstallStatus?.installed ?? true;
  const selectedProviderStatusResolved =
    selectedProviderInstallStatus !== null || !providerStatusLoading;
  const canCreate =
    selectedPathValue.length > 0 &&
    !isCreateBusy &&
    selectedProviderStatusResolved &&
    selectedProviderInstalled;

  const createStatusText =
    newThreadBindingStatus === "starting"
      ? "Starting terminal session..."
      : newThreadBindingStatus === "awaiting_discovery"
        ? "Session started. Waiting for first input to persist thread id..."
        : null;

  const visibleCreateError =
    createDialogError ?? (didAttemptCreate ? error : null);

  const loadProviderInstallStatuses = useCallback(
    async (projectPath: string) => {
      setProviderStatusLoading(true);
      setProviderStatusError(null);

      try {
        const normalizedProjectPath = sanitizeProjectPath(projectPath);
        const data = await invoke<ProviderInstallStatusPayload[]>(
          "list_provider_install_statuses",
          {
            projectPath: normalizedProjectPath.length > 0 ? normalizedProjectPath : null,
          },
        );

        const nextStatuses = emptyProviderInstallStatusMap();
        for (const item of data) {
          if (!isSupportedProvider(item.providerId)) {
            continue;
          }
          nextStatuses[item.providerId] = {
            providerId: item.providerId,
            installed: item.installed,
            healthStatus: item.healthStatus,
            message: item.message,
          };
        }
        setProviderInstallStatuses(nextStatuses);
      } catch (statusError) {
        const message =
          statusError instanceof Error ? statusError.message : String(statusError);
        setProviderStatusError(message);
      } finally {
        setProviderStatusLoading(false);
      }
    },
    [],
  );

  const handleOpenInstallGuide = useCallback(async () => {
    setProviderInstallGuideError(null);
    try {
      await openUrl(providerInstallGuideUrl(selectedProviderId));
    } catch (openError) {
      const message =
        openError instanceof Error ? openError.message : String(openError);
      setProviderInstallGuideError(message);
    }
  }, [selectedProviderId]);

  useEffect(() => {
    if (!settingsOpen && !themeDialogOpen && !newThreadDialogOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (themeDialogOpen || newThreadDialogOpen) {
        return;
      }
      if (!settingsOpen) {
        return;
      }
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }
      if (!settingsRef.current?.contains(target)) {
        setSettingsOpen(false);
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (themeDialogOpen) {
        setThemeDialogOpen(false);
        return;
      }
      if (newThreadDialogOpen) {
        if (!isCreateBusy) {
          setNewThreadDialogOpen(false);
        }
        return;
      }
      setSettingsOpen(false);
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [
    settingsOpen,
    themeDialogOpen,
    newThreadDialogOpen,
    isCreateBusy,
  ]);

  useEffect(() => {
    if (sidebarCollapsed) {
      setSettingsOpen(false);
      setThemeDialogOpen(false);
      setNewThreadDialogOpen(false);
    }
  }, [sidebarCollapsed]);

  useEffect(() => {
    if (!themeDialogOpen) {
      setPendingTheme(appTheme);
    }
  }, [appTheme, themeDialogOpen]);

  useEffect(() => {
    if (!newThreadDialogOpen || !createRequested) {
      return;
    }

    if (hasLaunchInFlight) {
      if (!didObserveLaunchState) {
        setDidObserveLaunchState(true);
      }
      if (newThreadBindingStatus === "awaiting_discovery") {
        setCreateRequested(false);
        setNewThreadDialogOpen(false);
      }
      return;
    }

    if (!didObserveLaunchState) {
      return;
    }

    if (visibleCreateError) {
      setCreateRequested(false);
      return;
    }

    setCreateRequested(false);
    setNewThreadDialogOpen(false);
  }, [
    newThreadDialogOpen,
    createRequested,
    newThreadBindingStatus,
    hasLaunchInFlight,
    didObserveLaunchState,
    visibleCreateError,
  ]);

  const selectedThemeOption = APP_THEME_OPTIONS.find(
    (option) => option.value === appTheme,
  );
  const pendingThemeOption = APP_THEME_OPTIONS.find(
    (option) => option.value === pendingTheme,
  );

  const openThemeDialog = () => {
    setPendingTheme(appTheme);
    setThemeDialogOpen(true);
    setSettingsOpen(false);
  };

  const applyThemeChange = () => {
    onAppThemeChange(pendingTheme);
    setThemeDialogOpen(false);
  };

  const openNewThreadDialog = () => {
    const fallbackProjectPath =
      (selectedFolderKey && selectedFolderKey !== "." ? selectedFolderKey : null)
      ?? folderGroups[0]?.key
      ?? "";

    setSelectedProjectPath(fallbackProjectPath);
    setSelectedProviderId("claude_code");
    setIsPickingFolder(false);
    setDidAttemptCreate(false);
    setCreateRequested(false);
    setDidObserveLaunchState(false);
    setPickerError(null);
    setCreateDialogError(null);
    setProviderInstallStatuses(emptyProviderInstallStatusMap());
    setProviderStatusError(null);
    setProviderInstallGuideError(null);
    onClearError();
    setNewThreadDialogOpen(true);
    void loadProviderInstallStatuses(fallbackProjectPath);
  };

  const closeNewThreadDialog = () => {
    if (isCreateBusy) {
      return;
    }
    setNewThreadDialogOpen(false);
  };

  const handlePickFolder = async () => {
    setPickerError(null);
    setCreateDialogError(null);
    onClearError();
    setIsPickingFolder(true);

    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: "Choose project folder",
      });
      const path = resolvePickedDirectory(picked);
      if (!path) {
        return;
      }
      const sanitizedPath = sanitizeProjectPath(path);
      setSelectedProjectPath(sanitizedPath);
      void loadProviderInstallStatuses(sanitizedPath);
    } catch (pickError) {
      const message =
        pickError instanceof Error ? pickError.message : String(pickError);
      setPickerError(message);
    } finally {
      setIsPickingFolder(false);
    }
  };

  const handleCreateFromDialog = async () => {
    if (!canCreate) {
      return;
    }

    const projectPath = sanitizeProjectPath(selectedProjectPath);
    if (!projectPath) {
      return;
    }

    setDidAttemptCreate(true);
    setCreateRequested(true);
    setDidObserveLaunchState(false);
    setCreateDialogError(null);
    setPickerError(null);
    onClearError();

    try {
      await onCreateThread(projectPath, selectedProviderId);
    } catch (createError) {
      const message =
        createError instanceof Error ? createError.message : String(createError);
      setCreateDialogError(message);
      setCreateRequested(false);
    }
  };

  if (sidebarCollapsed) {
    return null;
  }

  return (
    <Card className="flex min-h-0 flex-col rounded-none border-0 bg-card/92 shadow-none pt-8">
      <CardHeader className="px-4 py-3 pb-2.5">
        <div className="flex items-center gap-2 pt-1">
          <Button
            variant="default"
            size="sm"
            className="h-7 px-2.5 text-xs font-semibold shadow-sm hover:shadow"
            onClick={openNewThreadDialog}
            disabled={isCreateBusy}
            aria-label="Create a new thread"
          >
            {isCreateBusy ? (
              <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
            ) : (
              <Plus className="mr-1.5 h-3 w-3" />
            )}
            {isCreateBusy ? "Creating..." : "New Thread"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 px-2.5 text-xs"
            onClick={() => void onLoadThreads()}
            disabled={loadingThreads}
          >
            <RefreshCw className="mr-1.5 h-3 w-3" />
            Refresh
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-hidden py-2.5 pl-2.5 pr-2.5">
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-1.5 flex items-center justify-between px-0.5 pr-2.5">
            <p className="text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
              Projects
            </p>
            <Badge variant="secondary" className="h-5 px-2 text-[10px]">
              {folderGroups.length}
            </Badge>
          </div>

          {loadingThreads ? (
            <p className="px-1.5 py-2 text-xs text-muted-foreground">
              Loading threads...
            </p>
          ) : folderGroups.length === 0 ? (
            <p className="px-1.5 py-2 text-xs text-muted-foreground">
              No sessions found in <code>~/.claude/projects</code>, <code>~/.codex/sessions</code>, or{" "}
              <code>~/.local/share/opencode/storage/session</code>.
            </p>
          ) : (
            <div className="min-h-0 flex-1 overflow-hidden">
              <style>{`
                .sidebar-scroll::-webkit-scrollbar {
                  width: 6px;
                }
                .sidebar-scroll::-webkit-scrollbar-track {
                  background: transparent;
                  border: none;
                }
                .sidebar-scroll::-webkit-scrollbar-thumb {
                  background: hsl(var(--muted-foreground) / 0.4);
                  border-radius: 3px;
                }
                .sidebar-scroll::-webkit-scrollbar-thumb:hover {
                  background: hsl(var(--muted-foreground) / 0.6);
                }
              `}</style>
              <div className="sidebar-scroll h-full overflow-y-auto pr-2.5">
                <ul className="w-full space-y-2 pb-1.5">
                  {folderGroups.map((group) => (
                    <ThreadFolderGroup
                      key={group.key}
                      group={group}
                      isActiveFolder={group.key === selectedFolderKey}
                      selectedThreadId={selectedThreadId}
                      onSelectThread={onSelectThread}
                      onCreateThread={onCreateThread}
                      isCreatingThread={creatingThreadFolderKey === group.key}
                      formatLastActive={formatLastActive}
                      getPreview={threadPreview}
                    />
                  ))}
                </ul>
              </div>
            </div>
          )}
        </div>

        <div className="border-t border-border/70 pt-2">
          <div ref={settingsRef} className="relative">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-8 w-full items-center justify-between px-2.5 text-xs"
              onClick={() => setSettingsOpen((openState) => !openState)}
            >
              <span className="inline-flex items-center gap-1.5">
                <Settings2 className="h-3.5 w-3.5" />
                Settings
              </span>
              <ChevronUp
                className={cn(
                  "h-3.5 w-3.5 transition-transform",
                  settingsOpen ? "" : "rotate-180",
                )}
              />
            </Button>

            {settingsOpen ? (
              <div className="absolute bottom-full left-0 right-0 z-40 mb-2 rounded-md border border-border bg-card p-1.5 opacity-100 shadow-md">
                <p className="px-1.5 pb-1 text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Settings
                </p>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 w-full items-center justify-between px-2.5 text-xs"
                  onClick={openThemeDialog}
                >
                  <span className="inline-flex items-center gap-1.5">
                    {selectedThemeOption ? (
                      <selectedThemeOption.Icon className="h-3.5 w-3.5" />
                    ) : null}
                    App Theme
                  </span>
                  <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                    {selectedThemeOption?.label ?? "Light"}
                    <ChevronRight className="h-3.5 w-3.5" />
                  </span>
                </Button>
              </div>
            ) : null}
          </div>
        </div>
      </CardContent>

      {settingsOpen ? (
        <div
          className="fixed inset-0 z-30 bg-black/25"
          onClick={() => setSettingsOpen(false)}
        />
      ) : null}

      {newThreadDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={closeNewThreadDialog}
        >
          <Card
            className="w-full max-w-md border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Create New Thread</CardTitle>
              <CardDescription className="text-xs">
                Choose project folder first, then provider.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                  1. Choose Project
                </p>
                <div className="max-h-40 space-y-1 overflow-y-auto rounded-md border border-border/70 p-1.5">
                  {folderGroups.length === 0 ? (
                    <p className="px-1.5 py-1 text-xs text-muted-foreground">
                      No existing projects in current thread list.
                    </p>
                  ) : (
                    folderGroups.map((group) => {
                      const active = group.key === selectedPathValue;
                      return (
                        <Button
                          key={group.key}
                          type="button"
                          variant={active ? "secondary" : "ghost"}
                          size="sm"
                          className="h-8 w-full items-center justify-between px-2 text-xs"
                          onClick={() => {
                            setSelectedProjectPath(group.key);
                            setPickerError(null);
                            setCreateDialogError(null);
                            setProviderInstallGuideError(null);
                            onClearError();
                            void loadProviderInstallStatuses(group.key);
                          }}
                          disabled={isCreateBusy}
                        >
                          <span className="inline-flex min-w-0 items-center gap-1.5">
                            <Folder className="h-3.5 w-3.5 shrink-0" />
                            <span className="truncate">{group.folderName}</span>
                          </span>
                          <span className="text-[10px] text-muted-foreground">
                            {group.threads.length}
                          </span>
                        </Button>
                      );
                    })
                  )}
                </div>

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 w-full justify-start px-2 text-xs"
                  onClick={() => void handlePickFolder()}
                  disabled={isCreateBusy}
                >
                  {isPickingFolder ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <FolderSearch className="mr-1.5 h-3.5 w-3.5" />
                  )}
                  Choose local project
                </Button>

                {selectedPathValue ? (
                  <p className="text-[11px] text-muted-foreground">
                    Selected: <code>{selectedPathValue}</code>
                    {!hasSelectedFolderInList ? " (new)" : ""}
                  </p>
                ) : null}

                {pickerError ? (
                  <p className="text-[11px] text-destructive">{pickerError}</p>
                ) : null}
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                    2. Choose Provider
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 px-1.5 text-[10px]"
                    onClick={() => void loadProviderInstallStatuses(selectedPathValue)}
                    disabled={isCreateBusy || providerStatusLoading}
                  >
                    {providerStatusLoading ? (
                      <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                    ) : (
                      <RefreshCw className="mr-1 h-3 w-3" />
                    )}
                    Check
                  </Button>
                </div>
                <div className="grid grid-cols-1 gap-1.5">
                  {THREAD_PROVIDER_OPTIONS.map((provider) => {
                    const active = provider.value === selectedProviderId;
                    const status = providerInstallStatuses[provider.value];
                    return (
                      <Button
                        key={provider.value}
                        type="button"
                        variant={active ? "secondary" : "ghost"}
                        size="default"
                        className="h-auto w-full items-center justify-between px-2 py-1.5 text-xs"
                        onClick={() => {
                          setSelectedProviderId(provider.value);
                          setProviderInstallGuideError(null);
                        }}
                        disabled={isCreateBusy}
                      >
                        <span className="inline-flex min-w-0 items-center gap-1.5">
                          <provider.Icon className={cn("h-3.5 w-3.5", provider.accentClass)} />
                          <span className="truncate">{provider.label}</span>
                        </span>
                        <span className="inline-flex items-center gap-1.5">
                          <span
                            className={cn(
                              "text-[10px] font-medium",
                              providerStatusClass(status, providerStatusLoading),
                            )}
                          >
                            {providerStatusLabel(status, providerStatusLoading)}
                          </span>
                          {active ? <Check className="h-3.5 w-3.5" /> : null}
                        </span>
                      </Button>
                    );
                  })}
                </div>

                {providerStatusError ? (
                  <p className="text-[11px] text-destructive">
                    Failed to check provider status: {providerStatusError}
                  </p>
                ) : null}

                {selectedProviderInstallStatus && !selectedProviderInstallStatus.installed ? (
                  <div className="space-y-2 rounded border border-destructive/30 bg-destructive/10 px-2 py-2">
                    <p className="inline-flex items-center gap-1.5 text-[11px] text-destructive">
                      <AlertTriangle className="h-3.5 w-3.5" />
                      {providerDisplayName(selectedProviderId)} CLI is not installed.
                    </p>
                    {selectedProviderInstallStatus.message ? (
                      <p className="text-[11px] text-destructive/90">
                        {selectedProviderInstallStatus.message}
                      </p>
                    ) : null}
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 px-2 text-[11px]"
                      onClick={() => void handleOpenInstallGuide()}
                      disabled={isCreateBusy}
                    >
                      Install {providerDisplayName(selectedProviderId)}
                      <ExternalLink className="ml-1.5 h-3 w-3" />
                    </Button>
                  </div>
                ) : null}

                {selectedProviderInstallStatus?.installed &&
                selectedProviderInstallStatus.healthStatus === "degraded" ? (
                  <p className="text-[11px] text-amber-700 dark:text-amber-400">
                    {selectedProviderInstallStatus.message ?? "Provider is installed but setup is incomplete."}
                  </p>
                ) : null}

                {providerInstallGuideError ? (
                  <p className="text-[11px] text-destructive">
                    Failed to open install guide: {providerInstallGuideError}
                  </p>
                ) : null}
              </div>

              {createStatusText ? (
                <p className="text-[11px] text-muted-foreground">{createStatusText}</p>
              ) : null}

              {visibleCreateError ? (
                <p className="rounded border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-[11px] text-destructive">
                  {visibleCreateError}
                </p>
              ) : null}

              <div className="flex items-center justify-end gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={closeNewThreadDialog}
                  disabled={isCreateBusy}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="default"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => void handleCreateFromDialog()}
                  disabled={!canCreate}
                >
                  {isCreateBusy ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : null}
                  {!selectedProviderStatusResolved
                    ? "Checking..."
                    : selectedProviderInstalled
                      ? "Create"
                      : "Install Required"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      ) : null}

      {themeDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={() => setThemeDialogOpen(false)}
        >
          <Card
            className="w-full max-w-sm border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">App Theme</CardTitle>
              <CardDescription className="text-xs">
                Choose the appearance for the entire desktop app.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-2">
              {APP_THEME_OPTIONS.map(({ value, label, Icon }) => (
                <Button
                  key={value}
                  type="button"
                  variant={pendingTheme === value ? "secondary" : "ghost"}
                  size="sm"
                  className="h-9 w-full items-center justify-between px-2.5 text-xs"
                  onClick={() => setPendingTheme(value)}
                >
                  <span className="inline-flex items-center gap-1.5">
                    <Icon className="h-3.5 w-3.5" />
                    {label}
                  </span>
                  {pendingTheme === value ? <Check className="h-3.5 w-3.5" /> : null}
                </Button>
              ))}
              <div className="flex items-center justify-end gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => setThemeDialogOpen(false)}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={applyThemeChange}
                >
                  Apply
                </Button>
              </div>
              <p className="text-[11px] text-muted-foreground">
                Current: {pendingThemeOption?.label ?? "Light"}
              </p>
            </CardContent>
          </Card>
        </div>
      ) : null}
    </Card>
  );
}
