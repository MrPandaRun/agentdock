import {
  Bot,
  ChevronDown,
  ChevronRight,
  Code2,
  Folder,
  Loader2,
  SquarePen,
  SquareTerminal,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

import { ThreadListItem, type ThreadListThreadItem } from "./ThreadListItem";

export interface ThreadFolderGroupItem<T extends ThreadListThreadItem = ThreadListThreadItem> {
  key: string;
  folderName: string;
  threads: T[];
}

type ThreadProviderId = "claude_code" | "codex" | "opencode";

interface ThreadFolderGroupProps<T extends ThreadListThreadItem> {
  group: ThreadFolderGroupItem<T>;
  isActiveFolder: boolean;
  selectedThreadId: string | null;
  onSelectThread: (threadId: string) => void;
  onCreateThread: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
  isCreatingThread: boolean;
  formatLastActive: (raw: string) => string;
  getPreview: (thread: T) => string;
}

export function ThreadFolderGroup<T extends ThreadListThreadItem>({
  group,
  isActiveFolder,
  selectedThreadId,
  onSelectThread,
  onCreateThread,
  isCreatingThread,
  formatLastActive,
  getPreview,
}: ThreadFolderGroupProps<T>) {
  const [collapsed, setCollapsed] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (isActiveFolder) {
      setCollapsed(false);
    }
  }, [isActiveFolder]);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (target && rootRef.current?.contains(target)) {
        return;
      }
      setMenuOpen(false);
    };
    window.addEventListener("mousedown", handlePointerDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [menuOpen]);

  const handleCreate = async (providerId: ThreadProviderId) => {
    setMenuOpen(false);
    await onCreateThread(group.key, providerId);
  };

  return (
    <li className="w-full">
      <div ref={rootRef} className="w-full space-y-1">
        <div
          className={cn(
            "flex w-full items-center justify-between px-2 text-[11px] font-medium text-muted-foreground",
            isActiveFolder && "text-foreground",
          )}
        >
          <button
            type="button"
            className="inline-flex min-w-0 items-center gap-1 text-left hover:text-foreground"
            onClick={() => setCollapsed((current) => !current)}
            aria-expanded={!collapsed}
            aria-label={`${collapsed ? "Expand" : "Collapse"} folder ${group.folderName}`}
          >
            {collapsed ? (
              <ChevronRight className="h-3.5 w-3.5 shrink-0" />
            ) : (
              <ChevronDown className="h-3.5 w-3.5 shrink-0" />
            )}
            <Folder className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">{group.folderName}</span>
          </button>

          <div className="relative flex shrink-0 items-center gap-1">
            <span className="text-[10px]">{group.threads.length}</span>
            <button
              type="button"
              className={cn(
                "inline-flex h-5 w-5 items-center justify-center rounded border border-border/80 bg-card text-muted-foreground hover:text-foreground",
                isCreatingThread && "cursor-wait opacity-70",
              )}
              onClick={(event) => {
                event.stopPropagation();
                if (isCreatingThread) {
                  return;
                }
                setMenuOpen((current) => !current);
              }}
              aria-label={`Create a new thread in ${group.folderName}`}
            >
              {isCreatingThread ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <SquarePen className="h-3 w-3" />
              )}
            </button>

            {menuOpen ? (
              <div className="absolute right-0 top-6 z-30 min-w-[162px] overflow-hidden rounded-md border border-border bg-card p-1 text-card-foreground shadow-lg">
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[12px] hover:bg-accent"
                  onClick={() => void handleCreate("claude_code")}
                >
                  <Bot className="h-3.5 w-3.5 text-[hsl(var(--brand-claude))]" />
                  Claude Code
                </button>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[12px] hover:bg-accent"
                  onClick={() => void handleCreate("codex")}
                >
                  <SquareTerminal className="h-3.5 w-3.5 text-[hsl(var(--brand-codex))]" />
                  Codex
                </button>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[12px] hover:bg-accent"
                  onClick={() => void handleCreate("opencode")}
                >
                  <Code2 className="h-3.5 w-3.5 text-[hsl(var(--brand-opencode))]" />
                  OpenCode
                </button>
              </div>
            ) : null}
          </div>
        </div>

        {!collapsed ? (
          <ul className="box-border w-full space-y-0.5 pl-3">
            {group.threads.map((thread) => (
              <ThreadListItem
                key={thread.id}
                thread={thread}
                isActive={thread.id === selectedThreadId}
                onSelectThread={onSelectThread}
                formatLastActive={formatLastActive}
                getPreview={getPreview}
              />
            ))}
          </ul>
        ) : null}
      </div>
    </li>
  );
}
