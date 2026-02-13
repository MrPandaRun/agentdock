import { useEffect, useRef, useState } from "react";
import {
  Check,
  ChevronRight,
  ChevronUp,
  Monitor,
  Moon,
  RefreshCw,
  Settings2,
  Sun,
} from "lucide-react";

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
import { formatLastActive, threadPreview } from "@/lib/thread";
import { cn } from "@/lib/utils";
import type { AppTheme, ThreadProviderId } from "@/types";

interface AppThemeOption {
  value: AppTheme;
  label: string;
  Icon: typeof Sun;
}

const APP_THEME_OPTIONS: AppThemeOption[] = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
];

export interface SidebarProps {
  sidebarCollapsed: boolean;
  folderGroups: ThreadFolderGroupItem[];
  selectedFolderKey: string | null;
  selectedThreadId: string | null;
  loadingThreads: boolean;
  creatingThreadFolderKey: string | null;
  appTheme: AppTheme;
  onLoadThreads: () => void;
  onSelectThread: (threadId: string) => void;
  onCreateThread: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
  onAppThemeChange: (theme: AppTheme) => void;
}

export function Sidebar({
  sidebarCollapsed,
  folderGroups,
  selectedFolderKey,
  selectedThreadId,
  loadingThreads,
  creatingThreadFolderKey,
  appTheme,
  onLoadThreads,
  onSelectThread,
  onCreateThread,
  onAppThemeChange,
}: SidebarProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [themeDialogOpen, setThemeDialogOpen] = useState(false);
  const [pendingTheme, setPendingTheme] = useState<AppTheme>(appTheme);
  const settingsRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!settingsOpen && !themeDialogOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (themeDialogOpen) {
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
      if (event.key === "Escape") {
        if (themeDialogOpen) {
          setThemeDialogOpen(false);
          return;
        }
        setSettingsOpen(false);
      }
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [settingsOpen, themeDialogOpen]);

  useEffect(() => {
    if (sidebarCollapsed) {
      setSettingsOpen(false);
      setThemeDialogOpen(false);
    }
  }, [sidebarCollapsed]);

  useEffect(() => {
    if (!themeDialogOpen) {
      setPendingTheme(appTheme);
    }
  }, [appTheme, themeDialogOpen]);

  const selectedThemeOption = APP_THEME_OPTIONS.find((option) => option.value === appTheme);
  const pendingThemeOption = APP_THEME_OPTIONS.find((option) => option.value === pendingTheme);

  const openThemeDialog = () => {
    setPendingTheme(appTheme);
    setThemeDialogOpen(true);
    setSettingsOpen(false);
  };

  const applyThemeChange = () => {
    onAppThemeChange(pendingTheme);
    setThemeDialogOpen(false);
  };

  if (sidebarCollapsed) {
    return null;
  }

  return (
    <Card className="flex min-h-0 flex-col rounded-none border-0 bg-card/92 shadow-none pt-8">
      <CardHeader className="px-4 py-3 pb-2.5">
        <CardDescription className="text-[11px] uppercase tracking-[0.18em] text-primary/90">
          AgentDock
        </CardDescription>
        <CardTitle className="text-[22px] leading-none">Agent Console</CardTitle>
        <CardDescription className="text-xs">Thread switch + session relay</CardDescription>
        <div className="flex items-center gap-2 pt-1">
          <Button variant="secondary" size="sm" className="h-7 px-2.5 text-xs" disabled>
            New Thread
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
              Folders
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
              onClick={() => setSettingsOpen((open) => !open)}
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
                    {selectedThemeOption ? <selectedThemeOption.Icon className="h-3.5 w-3.5" /> : null}
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
