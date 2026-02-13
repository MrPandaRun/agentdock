import { RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  ThreadFolderGroup,
  type ThreadFolderGroupItem,
} from "@/components/threads/ThreadFolderGroup";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { ThreadProviderId } from "@/types";
import { formatLastActive, threadPreview } from "@/lib/thread";

export interface SidebarProps {
  sidebarCollapsed: boolean;
  folderGroups: ThreadFolderGroupItem[];
  selectedFolderKey: string | null;
  selectedThreadId: string | null;
  loadingThreads: boolean;
  creatingThreadFolderKey: string | null;
  onLoadThreads: () => void;
  onSelectThread: (threadId: string) => void;
  onCreateThread: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
}

export function Sidebar({
  sidebarCollapsed,
  folderGroups,
  selectedFolderKey,
  selectedThreadId,
  loadingThreads,
  creatingThreadFolderKey,
  onLoadThreads,
  onSelectThread,
  onCreateThread,
}: SidebarProps) {
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
      <CardContent className="flex min-h-0 flex-1 flex-col py-2.5 pl-2.5 pr-0">
        <div className="mb-1.5 flex items-center justify-between px-0.5">
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
          <div className="min-h-0 flex-1 overflow-y-auto">
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
            <div className="sidebar-scroll h-full">
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
      </CardContent>
    </Card>
  );
}
